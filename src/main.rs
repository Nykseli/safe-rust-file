use std::{env::args, ffi::c_void, os::fd::RawFd, path::Path};

use nix::{
    fcntl::{fcntl, open, openat, FcntlArg::F_OFD_SETLK, OFlag},
    libc::{
        flock, write, F_WRLCK, O_CLOEXEC, O_DIRECTORY, O_NOFOLLOW, O_PATH, O_RDONLY, O_RDWR,
        SEEK_SET, S_IFMT, S_IFREG, S_IWOTH,
    },
    sys::stat::{fstat, FileStat, Mode},
    unistd::close,
    NixPath,
};

struct File {
    fd: RawFd,
}

impl File {
    fn open<P>(path: &P, oflag: OFlag, mode: Mode) -> Result<Self, ()>
    where
        P: ?Sized + NixPath,
    {
        // TODO: errors
        let fd = open(path, oflag, mode).unwrap();
        Ok(Self { fd })
    }

    fn openat<P>(&self, path: &P, oflag: OFlag, mode: Mode) -> Result<Self, ()>
    where
        P: ?Sized + NixPath,
    {
        // TODO: errors
        let fd = openat(Some(self.fd), path, oflag, mode).unwrap();
        Ok(Self { fd })
    }

    pub fn fstat(&self) -> Result<FileStat, ()> {
        // TODO: return error
        Ok(fstat(self.fd).unwrap())
    }

    fn lock(&self) {
        let fl = flock {
            l_type: F_WRLCK as i16,
            l_whence: SEEK_SET as i16,
            l_start: 0,
            l_len: 0,
            l_pid: 0,
        };

        // TODO: return an error
        fcntl(self.fd, F_OFD_SETLK(&fl)).unwrap();
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = close(self.fd);
    }
}

struct SafeFile {
    fd: File,
}

impl SafeFile {
    fn walk_open_dir(dirs: &[&str]) -> File {
        let dir_flags = O_RDONLY | O_PATH | O_CLOEXEC | O_NOFOLLOW | O_DIRECTORY;
        let dir_flags = OFlag::from_bits(dir_flags).unwrap();
        let root_flags = O_PATH | O_DIRECTORY | O_CLOEXEC | O_RDONLY;
        let root_flags = OFlag::from_bits(root_flags).unwrap();

        // TODO: return error
        let mut parent_fd = File::open("/", root_flags, Mode::empty()).unwrap();

        // ignore the last as it should be a text file, not directory
        for dir in &dirs[..dirs.len() - 1] {
            // TODO: return error
            let next = parent_fd.openat(*dir, dir_flags, Mode::empty()).unwrap();

            // TODO: return error
            let stat = next.fstat().unwrap();

            // no world writeable files to pervent usafe paths like /tmp/ or /var/tmp/
            if stat.st_mode & S_IWOTH != 0 {
                panic!("No world writable files allowed");
            }

            parent_fd = next;
        }

        parent_fd
    }

    fn safe_reopen_file(file: File, flags: OFlag) -> File {
        let fd_path = format!("/proc/self/fd/{}", file.fd);
        // TODO: retun err
        let fd = File::open(fd_path.as_str(), flags, Mode::empty()).unwrap();
        fd
    }

    fn safe_open_file(parent: File, name: &str) -> File {
        let oflags = OFlag::from_bits(O_RDONLY | O_PATH | O_CLOEXEC | O_NOFOLLOW).unwrap();
        // TODO: return error
        let fd = parent.openat(name, oflags, Mode::empty()).unwrap();
        // TODO: return err if stat files
        let stat = fd.fstat().unwrap();

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
        let parent_fd = Self::walk_open_dir(&dirs[1..]);
        let fd = Self::safe_open_file(parent_fd, dirs.iter().last().unwrap());
        fd.lock();

        Ok(Self { fd })
    }

    fn write(&self, data: &str) {
        // TODO: Make sure the whole buffer is written and retun err if it's not
        unsafe {
            write(self.fd.fd, data.as_ptr() as *const c_void, data.len());
        }
    }
}

fn main() {
    let args: Vec<String> = args().collect();

    if args.len() < 2 {
        println!("Give file path as an argument");
        return;
    }

    let path_full = Path::new(&args[1]).canonicalize().unwrap();

    let file = SafeFile::open(path_full.to_str().unwrap()).unwrap();
    file.write("Hello world!\n");
}
