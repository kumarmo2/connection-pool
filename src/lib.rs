use std::sync::{Arc, Mutex};

pub trait ConnectionConnector {
    type Conn: Connection;
    // TODO:  Will need to change the Option to Result
    fn connect(&self) -> Option<Self::Conn>;
}

pub trait Connection {
    fn is_alive(&self) -> bool;
}

trait ConnectionPool {
    type C: Connection;
    type CC: ConnectionConnector<Conn = Self::C>;

    fn get_connection(&mut self) -> Option<&Self::C>;
}

pub struct GenericConnectionPool<E>
where
    E: ConnectionConnector,
{
    _max_connections: u8,
    _min_connections: u8,
    _connections: Arc<Mutex<Vec<<E as ConnectionConnector>::Conn>>>,
    // _in_use_connections: Arc<Mutex<Vec<<E as ConnectionConnector>::Conn>>>,
    _connector: E,
}

impl<E> ConnectionPool for GenericConnectionPool<E>
where
    E: ConnectionConnector,
{
    type CC = E;
    type C = <E as ConnectionConnector>::Conn;

    fn get_connection(&mut self) -> Option<&Self::C> {
        // let cloned = Arc::clone(&self._connections);
        // // let guard = cloned.lock().unwrap();
        // // let x = guard.get(0);
        // let x = cloned.lock().unwrap().pop().unwrap();
        // let mut y = self._in_use_connections.clone().lock().unwrap();
        // y.push(x);
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::{Connection, ConnectionConnector, GenericConnectionPool};
    use std::sync::{Arc, Mutex};

    #[test]
    fn connector_works() {
        struct DummyConnection {};

        impl Connection for DummyConnection {
            fn is_alive(&self) -> bool {
                true
            }
        }
        struct DummyConnectionConnector {};
        impl ConnectionConnector for DummyConnectionConnector {
            type Conn = DummyConnection;

            fn connect(&self) -> Option<Self::Conn> {
                Some(DummyConnection {})
            }
        }

        let cc = DummyConnectionConnector {};

        let min = 1;

        let connections: Vec<DummyConnection> = (0..min).map(|_| cc.connect().unwrap()).collect();

        GenericConnectionPool::<DummyConnectionConnector> {
            _max_connections: 4,
            _min_connections: 1,
            _connections: Arc::new(Mutex::new(connections)),
            _connector: cc,
        };
    }
}
