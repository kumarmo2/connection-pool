use std::{
    future::Future,
    mem::{self, MaybeUninit},
    ops::Deref,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
};

pub trait ConnectionConnector {
    type Conn: Connection;
    type Future: Future<Output = Option<Self::Conn>>;
    // TODO:  Will need to change the Option to Result
    fn connect(&self) -> Self::Future;
}

pub trait Connection {
    type Output: Future<Output = bool>;
    fn is_alive(&self) -> Self::Output;
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
    pub async fn get_connection<'a>(&'a self) -> Option<LiveConnection<'a, E>> {
        let conn;
        let mut guard = self._num_of_live_connections.lock().unwrap();
        let num_of_connections = *guard;
        loop {
            // println!("num of connections: {}", num_of_connections);
            if num_of_connections < self._max_connections {
                // println!("making a new connection");
                match self._connector.connect().await {
                    Some(c) => {
                        conn = c;
                        *guard = *guard + 1;
                        break;
                    }
                    None => {}
                }
            } else {
                let receiver = Arc::clone(&self._reciever);

                // TODO: Think if we really need to wrap the receiver in a Mutex
                // as we are using Receiver.recv() in a this method only and which is already
                // ensured that only one thread will be able to execute this method because of
                // _num_of_live_connections mutex encapsulates the whole method.

                let guarded_reciever = receiver.lock().unwrap();
                // println!("going to listen on queue");
                if let Ok(local_conn) = guarded_reciever.recv() {
                    // println!("value recieved from queue");
                    if local_conn.is_alive().await {
                        // println!("reusing connection");
                        conn = local_conn;
                        break;
                    } else {
                        // println!("inside else block");
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

// #[cfg(test)]
// mod tests {
//     use super::{Connection, ConnectionConnector, GenericConnectionPool};
//     use std::thread;
//     use std::time::Duration;

//     #[test]
//     fn connector_works() {
//         struct DummyConnection {};

//         impl Connection for DummyConnection {
//             fn is_alive(&self) -> bool {
//                 true
//             }
//         }
//         #[derive(Clone)]
//         struct DummyConnectionConnector {};
//         impl ConnectionConnector for DummyConnectionConnector {
//             type Conn = DummyConnection;

//             fn connect(&self) -> Option<Self::Conn> {
//                 Some(DummyConnection {})
//             }
//         }

//         let cc = DummyConnectionConnector {};

//         let pool = GenericConnectionPool::new(2, cc);
//         println!("here");
//         {
//             for _ in 0..5 {
//                 let pool = pool.clone();
//                 std::thread::spawn(move || {
//                     let parent_thread_id = thread::current().id();
//                     println!("threadId: {:?}", parent_thread_id);
//                     let conn = pool.get_connection().unwrap();
//                     println!(
//                         "got conection for parentId: {:?} and now sleeping for 2 seconds",
//                         parent_thread_id
//                     );
//                     thread::sleep(Duration::from_secs(2));
//                     println!("awoken: {:?}", parent_thread_id);
//                 });
//             }
//         }
//         println!("waiting for 20 seconds");
//         thread::sleep(Duration::from_secs(20));
//     }
// }
