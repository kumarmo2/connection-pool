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
        // println!("inside drop");
        let zeroed_mem = MaybeUninit::<<T as ConnectionConnector>::Conn>::zeroed();
        // println!("zeroed memory initialized");
        let zeroed_mem = unsafe { zeroed_mem.assume_init() };
        // println!("zeroed memory after assume_init");
        let old_val = mem::replace(&mut self.conn, zeroed_mem);
        // println!("after replace call");
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
        let num_of_connections = *guard;
        loop {
            println!("num of connections: {}", num_of_connections);
            if num_of_connections < self._max_connections {
                println!("making a new connection");
                conn = self._connector.connect().unwrap();
                *guard = *guard + 1;
                break;
            } else {
                let receiver = Arc::clone(&self._reciever);

                // TODO: Think if we really need to wrap the receiver in a Mutex
                // as we are using Receiver.recv() in a this method only and which is already
                // ensured that only one thread will be able to execute this method because of
                // _num_of_live_connections mutex encapsulates the whole method.

                let guarded_reciever = receiver.lock().unwrap();
                println!("going to listen on queue");
                if let Ok(local_conn) = guarded_reciever.recv() {
                    println!("value recieved from queue");
                    if local_conn.is_alive() {
                        println!("reusing connection");
                        conn = local_conn;
                        break;
                    } else {
                        println!("inside else block");
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

#[cfg(test)]
mod tests {
    use super::{Connection, ConnectionConnector, GenericConnectionPool};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn connector_works() {
        struct DummyConnection {};

        impl Connection for DummyConnection {
            fn is_alive(&self) -> bool {
                true
            }
        }
        #[derive(Clone)]
        struct DummyConnectionConnector {};
        impl ConnectionConnector for DummyConnectionConnector {
            type Conn = DummyConnection;

            fn connect(&self) -> Option<Self::Conn> {
                Some(DummyConnection {})
            }
        }

        let cc = DummyConnectionConnector {};

        let pool = GenericConnectionPool::new(2, cc);
        println!("here");
        {
            for _ in 0..5 {
                let pool = pool.clone();
                std::thread::spawn(move || {
                    let parent_thread_id = thread::current().id();
                    println!("threadId: {:?}", parent_thread_id);
                    let conn = pool.get_connection().unwrap();
                    println!(
                        "got conection for parentId: {:?} and now sleeping for 2 seconds",
                        parent_thread_id
                    );
                    thread::sleep(Duration::from_secs(2));
                    println!("awoken: {:?}", parent_thread_id);
                });
            }
        }
        println!("waiting for 20 seconds");
        thread::sleep(Duration::from_secs(20));
    }
}
