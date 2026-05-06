use std::{fs, io};

use interprocess::local_socket::{GenericFilePath, Name, ToFsName};

use crate::SocketAddress;

pub fn prepare_bind_address(address: &SocketAddress) -> io::Result<()> {
    match address {
        SocketAddress::Unix(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            remove_stale_socket(path)?;
            Ok(())
        }
        SocketAddress::NamedPipe(path) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("named pipes are not supported on unix hosts: {path}"),
        )),
    }
}

pub fn listener_name(address: &SocketAddress) -> io::Result<Name<'static>> {
    match address {
        SocketAddress::Unix(path) => path.clone().to_fs_name::<GenericFilePath>(),
        SocketAddress::NamedPipe(path) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("named pipes are not supported on unix hosts: {path}"),
        )),
    }
}

pub fn stream_name(address: &SocketAddress) -> io::Result<Name<'static>> {
    listener_name(address)
}

fn remove_stale_socket(path: &std::path::Path) -> io::Result<()> {
    use std::os::unix::fs::FileTypeExt;
    use std::os::unix::net::UnixStream;

    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };

    if !metadata.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("refusing to replace non-socket path {}", path.display()),
        ));
    }

    match UnixStream::connect(path) {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            format!(
                "refusing to replace live unix socket {}; daemon already running",
                path.display()
            ),
        )),
        Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => remove_socket_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            format!(
                "refusing to replace existing unix socket {} because liveness probe did not prove it stale: {error}",
                path.display()
            ),
        )),
    }
}

fn remove_socket_file(path: &std::path::Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
