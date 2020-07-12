use std::{
    mem::{self, MaybeUninit},
    ops::Deref,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
};

pub trait ConnectionConnector {
    type Conn: Connection;
    // TODO:  Will need to change the Option to Result
    fn connect(&self) -> Option<Self::Conn>;
}

pub trait Connection {
    fn is_alive(&self) -> bool;
}

pub struct LiveConnection<'a, T>
where
    T: ConnectionConnector + Clone,
{
    conn: <T as ConnectionConnector>::Conn,
    pool: &'a GenericConnectionPool<T>,
}

impl<'a, T> Deref for LiveConnection<'a, T>
where
    T: ConnectionConnector + Clone,
{
    type Target = T::Conn;
    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

impl<'a, T> Drop for LiveConnection<'a, T>
where
    T: ConnectionConnector + Clone,
{
    fn drop(&mut self) {
        //TODO: Move this logic to GenericConnectionPool.
        let zeroed_mem = MaybeUninit::<<T as ConnectionConnector>::Conn>::zeroed();
        let zeroed_mem = unsafe { zeroed_mem.assume_init() };
        let old_val = mem::replace(&mut self.conn, zeroed_mem);
        self.pool._sender.send(old_val);
    }
}

#[derive(Clone)]
pub struct GenericConnectionPool<E>
where
    E: ConnectionConnector + Clone,
{
    _sender: Sender<<E as ConnectionConnector>::Conn>,
    _reciever: Arc<Mutex<Receiver<<E as ConnectionConnector>::Conn>>>,
    // To Keep the GenericConnectionPool Cloneable, couldn't use the AtomicU8.
    _num_of_live_connections: Arc<Mutex<u8>>,
    _max_connections: u8,
    _connector: E,
}

impl<E> GenericConnectionPool<E>
where
    E: ConnectionConnector + Clone,
{
    pub fn new(max_connections: u8, connector: E) -> Self {
        let (sender, receiver) = mpsc::channel::<<E as ConnectionConnector>::Conn>();
        let pool = Self {
            _sender: sender,
            _num_of_live_connections: Arc::new(Mutex::new(0)),
            _reciever: Arc::new(Mutex::new(receiver)),
            _max_connections: max_connections,
            _connector: connector,
        };
        pool
    }
}

impl<E> GenericConnectionPool<E>
where
    E: ConnectionConnector + Clone,
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
                    if c.is_alive() {
                        conn = c;
                        break;
                    } else {
                        if *guard > 0 {
                            *guard = *guard - 1;
                        }
                    }
                }
                Err(_) => {}
            }

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
                    if local_conn.is_alive() {
                        conn = local_conn;
                        break;
                    } else {
                        if *guard > 0 {
                            *guard = *guard - 1;
                        }
                    }
                }
            }
        }
        Some(LiveConnection { conn, pool: self })
    }
}
