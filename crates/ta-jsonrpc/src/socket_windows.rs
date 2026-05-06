use std::io;

use interprocess::local_socket::{GenericNamespaced, Name, ToNsName};

use crate::SocketAddress;

pub fn prepare_bind_address(address: &SocketAddress) -> io::Result<()> {
    match address {
        SocketAddress::NamedPipe(_) => Ok(()),
        SocketAddress::Unix(path) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "unix domain sockets are not supported on windows hosts: {}",
                path.display()
            ),
        )),
    }
}

pub fn listener_name(address: &SocketAddress) -> io::Result<Name<'static>> {
    match address {
        SocketAddress::NamedPipe(path) => path.clone().to_ns_name::<GenericNamespaced>(),
        SocketAddress::Unix(path) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "unix domain sockets are not supported on windows hosts: {}",
                path.display()
            ),
        )),
    }
}

pub fn stream_name(address: &SocketAddress) -> io::Result<Name<'static>> {
    listener_name(address)
}
