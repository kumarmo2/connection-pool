use scheduled_thread_pool::ScheduledThreadPool;
use std::{
    mem::{self, MaybeUninit},
    ops::Deref,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub trait Connection {
    fn is_alive(&self) -> bool;
}

pub trait ConnectionConnector {
    type Conn: Connection;
    // TODO:  Will need to change the Option to Result
    fn connect(&self) -> Option<Self::Conn>;
}

struct PoolEntry<T>
where
    T: ConnectionConnector,
{
    _conn: <T as ConnectionConnector>::Conn,
    _idle_start_instant: Option<Instant>,
}

pub struct LiveConnection<'a, T>
where
    T: ConnectionConnector + Clone + Send,
    <T as ConnectionConnector>::Conn: Send,
{
    _conn: <T as ConnectionConnector>::Conn,
    _pool: &'a GenericConnectionPool<T>,
}

impl<'a, T> Deref for LiveConnection<'a, T>
where
    T: ConnectionConnector + Clone + Send,
    <T as ConnectionConnector>::Conn: Send,
{
    type Target = T::Conn;
    fn deref(&self) -> &Self::Target {
        &self._conn
    }
}

impl<'a, T> Drop for LiveConnection<'a, T>
where
    T: ConnectionConnector + Clone + Send,
    <T as ConnectionConnector>::Conn: Send,
{
    fn drop(&mut self) {
        //TODO: Move this logic to GenericConnectionPool.
        let zeroed_mem = MaybeUninit::<<T as ConnectionConnector>::Conn>::zeroed();
        let zeroed_mem = unsafe { zeroed_mem.assume_init() };
        let old_val = mem::replace(&mut self._conn, zeroed_mem);
        let pool_entry = PoolEntry {
            _conn: old_val,
            _idle_start_instant: Some(Instant::now()),
        };
        self._pool._sender.send(pool_entry);
    }
}

#[derive(Clone)]
// TODO: Make an Inner Type and wrap it in the Arc, to remove the constraint for E to be cloneable.
pub struct GenericConnectionPool<E>
where
    E: ConnectionConnector + Clone + Send,
    <E as ConnectionConnector>::Conn: Send,
{
    // _sender: Sender<<E as ConnectionConnector>::Conn>,
    _sender: Sender<PoolEntry<E>>,
    // _reciever: Arc<Mutex<Receiver<<E as ConnectionConnector>::Conn>>>,
    _reciever: Arc<Mutex<Receiver<PoolEntry<E>>>>,
    _max_idle_duration: Duration,
    // To Keep the GenericConnectionPool Cloneable, couldn't use the AtomicU8.
    _num_of_live_connections: Arc<Mutex<u8>>,
    _max_connections: u8,
    _min_connections: u8,
    thread_pool: Arc<ScheduledThreadPool>,
    _connector: E,
}

impl<E> GenericConnectionPool<E>
where
    E: ConnectionConnector + Clone + Send + 'static,
    <E as ConnectionConnector>::Conn: Send,
{
    pub fn new(
        max_connections: u8,
        min_connections: u8,
        max_idle_duration: Duration,
        connector: E,
    ) -> Self {
        assert!(min_connections > 0);
        assert!(max_connections >= min_connections);

        let (sender, receiver) = mpsc::channel::<PoolEntry<E>>();
        let thread_pool = ScheduledThreadPool::new(1);
        let reciever = Arc::new(Mutex::new(receiver));
        let num_of_live_connections = Arc::new(Mutex::new(0));
        let thread_pool = Arc::new(thread_pool);

        let pool = Self {
            _sender: sender.clone(),
            _num_of_live_connections: Arc::clone(&num_of_live_connections),
            _reciever: Arc::clone(&reciever),
            _max_connections: max_connections,
            _min_connections: min_connections,
            _max_idle_duration: max_idle_duration,
            _connector: connector,
            thread_pool: Arc::clone(&thread_pool),
        };

        let thread_pool = Arc::clone(&thread_pool);
        let sender = sender.clone();
        let reciever = Arc::clone(&reciever);
        let num_of_live_connections = Arc::clone(&num_of_live_connections);

        thread_pool.execute_at_fixed_rate(
            pool._max_idle_duration,
            pool._max_idle_duration,
            move || {
                println!("job executed");
                let reciever = reciever.lock().unwrap();
                loop {
                    let mut connections = num_of_live_connections.lock().unwrap();
                    println!("num of connections: {}", *connections);
                    if *connections <= min_connections {
                        break;
                    }
                    let conn = reciever.recv().unwrap();
                    if conn._conn.is_alive()
                        && conn._idle_start_instant.as_ref().unwrap().elapsed() < max_idle_duration
                    {
                        println!("giving back");
                        sender.send(conn);
                        break;
                    } else {
                        println!("killing the connection");
                        *connections = *connections - 1;
                    }
                }
            },
        );
        pool
    }
}

impl<E> GenericConnectionPool<E>
where
    E: ConnectionConnector + Clone + Send,
    <E as ConnectionConnector>::Conn: Send,
{
    pub fn get_connection(&self) -> Option<LiveConnection<E>> {
        let conn;
        let mut guard = self._num_of_live_connections.lock().unwrap();
        let receiver = Arc::clone(&self._reciever);
        let guarded_reciever = receiver.lock().unwrap();
        let num_of_connections = *guard;
        loop {
            match guarded_reciever.try_recv() {
                Ok(c) => {
                    if c._conn.is_alive() {
                        // println!("try_recv: reusing, total conns: {}", num_of_connections);
                        conn = c._conn;
                        break;
                    } else {
                        if *guard > 0 {
                            *guard = *guard - 1;
                        }
                    }
                }
                Err(_) => {
                    if num_of_connections < self._max_connections {
                        // Making a new connection
                        match self._connector.connect() {
                            Some(c) => {
                                conn = c;
                                *guard = *guard + 1;
                                break;
                            }
                            None => {}
                        }
                    } else {
                        // TODO: Think if we really need to wrap the receiver in a Mutex
                        // as we are using Receiver.recv() in a this method only and which is already
                        // ensured that only one thread will be able to execute this method because of
                        // _num_of_live_connections mutex encapsulates the whole method.

                        // Blocking on queue
                        if let Ok(local_conn) = guarded_reciever.recv() {
                            if local_conn._conn.is_alive() {
                                conn = local_conn._conn;
                                break;
                            } else {
                                if *guard > 0 {
                                    *guard = *guard - 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(LiveConnection {
            _conn: conn,
            _pool: self,
        })
    }
}
