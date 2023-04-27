use crate::error::*;
use std::io::{Read, BufWriter};
use std::io;
use std::ops;
use std::process::ChildStdin;
use std::process::{Command, Stdio};

enum Writer {
    #[cfg(feature = "lzma")]
    Xz(xz2::write::XzEncoder<Vec<u8>>),
    StdIn(BufWriter<ChildStdin>),
    #[cfg(not(feature = "lzma"))]
    Gz(flate2::write::GzEncoder<Vec<u8>>),
}

pub struct Compressor {
    writer: Writer,
    ret: Box<dyn FnOnce(Writer) -> io::Result<Compressed> + Send + Sync>,
    pub uncompressed_size: usize,
}

impl io::Write for Compressor {
    fn flush(&mut self) -> io::Result<()> {
        match &mut self.writer {
            #[cfg(feature = "lzma")]
            Writer::Xz(w) => w.flush(),
            #[cfg(not(feature = "lzma"))]
            Writer::Gz(w) => w.flush(),
            Writer::StdIn(w) => w.flush(),
        }
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = match &mut self.writer {
            #[cfg(feature = "lzma")]
            Writer::Xz(w) => w.write(buf),
            #[cfg(not(feature = "lzma"))]
            Writer::Gz(w) => w.write(buf),
            Writer::StdIn(w) => w.write(buf),
        }?;
        self.uncompressed_size += len;
        Ok(len)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match &mut self.writer {
            #[cfg(feature = "lzma")]
            Writer::Xz(w) => w.write_all(buf),
            #[cfg(not(feature = "lzma"))]
            Writer::Gz(w) => w.write_all(buf),
            Writer::StdIn(w) => w.write_all(buf),
        }?;
        self.uncompressed_size += buf.len();
        Ok(())
    }
}

impl Compressor {
    fn new(writer: Writer, ret: impl FnOnce(Writer) -> io::Result<Compressed> + Send + Sync + 'static) -> Self {
        Self {
            writer,
            ret: Box::new(ret),
            uncompressed_size: 0,
        }
    }

    pub fn finish(self) -> CDResult<Compressed> {
        (self.ret)(self.writer).map_err(From::from)
    }
}

pub enum Compressed {
    Gz(Vec<u8>),
    Xz(Vec<u8>),
}

impl ops::Deref for Compressed {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Gz(data) | Self::Xz(data) => data,
        }
    }
}

impl Compressed {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Gz(_) => "gz",
            Self::Xz(_) => "xz",
        }
    }
}

fn system_xz(fast: bool) -> CDResult<Compressor> {
    let mut child = Command::new("xz")
        .arg(if fast { "-1" } else { "-6" })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| CargoDebError::CommandFailed(e, "xz"))?;
    let mut stdout = child.stdout.take().unwrap();

    let t = std::thread::spawn(move || {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).map(|_| buf)
    });

    let stdin = BufWriter::with_capacity(1<<16, child.stdin.take().unwrap());
    Ok(Compressor::new(Writer::StdIn(stdin), move |stdin| {
        drop(stdin);
        child.wait()?;
        t.join().unwrap().map(Compressed::Xz)
    }))
}

/// Compresses data using the [native Rust implementation of Zopfli](https://github.com/carols10cents/zopfli).
#[cfg(not(feature = "lzma"))]
pub fn xz_or_gz(fast: bool, with_system_xz: bool) -> CDResult<Compressor> {
    // Very old dpkg doesn't support LZMA, so use it only if expliclty enabled
    if with_system_xz {
        return system_xz(fast);
    }

    use flate2::Compression;
    use flate2::write::GzEncoder;

    let writer = GzEncoder::new(Vec::new(), if fast { Compression::fast() } else { Compression::best() });

    Ok(Compressor::new(Writer::Gz(writer), move |writer| {
        match writer {
            Writer::Gz(w) => Ok(Compressed::Gz(w.finish()?)),
            _ => unreachable!(),
        }
    }))
}

/// Compresses data using the xz2 library
#[cfg(feature = "lzma")]
pub fn xz_or_gz(fast: bool, with_system_xz: bool) -> CDResult<Compressor> {
    if with_system_xz {
        return system_xz(fast);
    }

    // Compression level 6 is a good trade off between size and [ridiculously] long compression time
    let encoder = xz2::stream::MtStreamBuilder::new()
        .threads(num_cpus::get() as u32)
        .preset(if fast { 1 } else { 6 })
        .encoder()
        .map_err(CargoDebError::LzmaCompressionError)?;

    let writer = xz2::write::XzEncoder::new_stream(Vec::new(), encoder);

    Ok(Compressor::new(Writer::Xz(writer), |writer| {
        match writer {
            Writer::Xz(w) => w.finish().map(Compressed::Xz),
            _ => unreachable!(),
        }
    }))
}
