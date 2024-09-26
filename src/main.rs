use std::{ffi::c_void, os::fd::RawFd};

use nix::{
    fcntl::{open, openat, OFlag},
    libc::{
        write, O_CLOEXEC, O_DIRECTORY, O_NOFOLLOW, O_PATH, O_RDONLY, O_RDWR, S_IFMT, S_IFREG,
        S_IWOTH,
    },
    sys::stat::{fstat, Mode},
    unistd::close,
};

struct SafeFile {
    fd: RawFd,
}

impl SafeFile {
    fn walk_open_dir(dirs: &[&str]) -> RawFd {
        let dir_flags = O_RDONLY | O_PATH | O_CLOEXEC | O_NOFOLLOW | O_DIRECTORY;
        let dir_flags = OFlag::from_bits(dir_flags).unwrap();
        let root_flags = O_PATH | O_DIRECTORY | O_CLOEXEC | O_RDONLY;
        let root_flags = OFlag::from_bits(root_flags).unwrap();

        // TODO: return error
        let mut parent_fd = open("/", root_flags, Mode::empty()).unwrap();

        // ignore the last as it should be a text file, not directory
        for dir in &dirs[..dirs.len() - 1] {
            // TODO: return error
            let next = openat(Some(parent_fd), *dir, dir_flags, Mode::empty()).unwrap();

            // TODO: return error
            let stat = fstat(next).unwrap();

            // no world writeable files to pervent usafe paths like /tmp/ or /var/tmp/
            if stat.st_mode & S_IWOTH != 0 {
                panic!("No world writable files allowed");
            }

            close(parent_fd).unwrap();
            parent_fd = next;
        }

        parent_fd
    }

    fn safe_reopen_file(file: RawFd, flags: OFlag) -> RawFd {
        let fd_path = format!("/proc/self/fd/{file}");
        // TODO: retun err
        let fd = open(fd_path.as_str(), flags, Mode::empty()).unwrap();
        close(file).unwrap();
        fd
    }

    fn safe_open_file(parent: RawFd, name: &str) -> RawFd {
        let oflags = OFlag::from_bits(O_RDONLY | O_PATH | O_CLOEXEC | O_NOFOLLOW).unwrap();
        // TODO: return error
        let fd = openat(Some(parent), name, oflags, Mode::empty()).unwrap();
        // TODO: return err if stat files
        let stat = fstat(fd).unwrap();

        // no world writeable or special user files
        if stat.st_mode & S_IWOTH != 0 || stat.st_mode & S_IFMT != S_IFREG {
            // TODO: return error
            panic!("No world writable files or sepcial user files allowed");
        }

        let oflags = OFlag::from_bits(O_RDWR | O_CLOEXEC).unwrap();
        Self::safe_reopen_file(fd, oflags)
    }

    fn open(path: &str) -> Result<Self, ()> {
        // only allow full paths
        assert_eq!(&path[..1], "/");

        let dirs: Vec<&str> = path.split('/').collect();

        for dir in &dirs {
            if *dir == ".." || *dir == "." {
                panic!("no relative paths");
            }
        }

        // first item will be empty string so ignore it
        // TODO: parent_fd should be struct that can be auto dropped
        let parent_fd = Self::walk_open_dir(&dirs[1..]);
        let fd = Self::safe_open_file(parent_fd, dirs.iter().last().unwrap());
        close(parent_fd).unwrap();

        Ok(Self { fd })
    }

    fn write(&self, data: &str) {
        // TODO: Make sure the whole buffer is written and retun err if it's not
        unsafe {
            write(self.fd, data.as_ptr() as *const c_void, data.len());
        }
    }
}

impl Drop for SafeFile {
    fn drop(&mut self) {
        let _ = close(self.fd);
    }
}

fn main() {
    let file =
        SafeFile::open("/home/duck/Documents/rust-playground/safe-file/tmp/test.txt").unwrap();
    file.write("Hello world!\n");
}
