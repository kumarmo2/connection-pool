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
        println!("drop of LiveConnection");
        let x = MaybeUninit::<<T as ConnectionConnector>::Conn>::zeroed();
        let x = unsafe { x.assume_init() };
        let real = mem::replace(&mut self.conn, x);
        println!("In drop of LiveConnection, reclaiming the connection");
        // guard.push(real);
        self.pool._sender.send(real);
    }
}

#[derive(Clone)]
pub struct GenericConnectionPool<E>
where
    E: ConnectionConnector + Clone,
{
    _sender: Sender<<E as ConnectionConnector>::Conn>,
    //TODO: Think do we really need the Arc here?
    _reciever: Arc<Mutex<Receiver<<E as ConnectionConnector>::Conn>>>,
    _num_of_live_connections: Arc<Mutex<u8>>,
    _max_connections: u8,
    _min_connections: u8,
    _connector: E,
}

impl<E> GenericConnectionPool<E>
where
    E: ConnectionConnector + Clone,
{
    pub fn new(max_connections: u8, min_connections: u8, connector: E) -> Self {
        let (sender, receiver) = mpsc::channel::<<E as ConnectionConnector>::Conn>();
        let pool = Self {
            _sender: sender,
            _num_of_live_connections: Arc::new(Mutex::new(0)),
            _reciever: Arc::new(Mutex::new(receiver)),
            _max_connections: max_connections,
            _min_connections: min_connections,
            _connector: connector,
        };
        pool
    }
}

impl<E> GenericConnectionPool<E>
where
    E: ConnectionConnector + Clone,
{
    //TODO: Respect the max and min connections constraints.
    pub fn get_connection(&self) -> Option<LiveConnection<E>> {
        let conn;
        loop {
            let num_of_connections;
            {
                let guard = self._num_of_live_connections.lock().unwrap();
                num_of_connections = *guard;
            }
            println!("num of connections: {}", num_of_connections);
            if num_of_connections < self._max_connections {
                println!("making a new connection");
                conn = self._connector.connect().unwrap();
                {
                    let mut guard = self._num_of_live_connections.lock().unwrap();
                    *guard = *guard + 1;
                }
                break;
            } else {
                let receiver = Arc::clone(&self._reciever);
                let guarded_reciever = receiver.lock().unwrap();
                println!("going to listen on queue");
                if let Ok(local_conn) = guarded_reciever.recv() {
                    println!("value recieved from queue");
                    if local_conn.is_alive() {
                        println!("reusing connection");
                        conn = local_conn;
                        break;
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

        let pool = GenericConnectionPool::new(2, 1, cc);
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
