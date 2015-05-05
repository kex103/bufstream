// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A crate for separately buffered streams.
//!
//! This crate provides a `BufStream` type which provides buffering of both the
//! reading and writing halves of a `Read + Write` type. Each half is completely
//! independently buffered of the other, which may not always be desired. For
//! example `BufStream<File>` may have surprising semantics.
//!
//! # Usage
//!
//! ```toml
//! [dependencies]
//! bufstream = "0.1"
//! ```
//!
//! ```no_run
//! use std::io::prelude::*;
//! use std::net::TcpStream;
//! use bufstream::BufStream;
//!
//!
//! let stream = TcpStream::connect("localhost:4000").unwrap();
//! let mut buf = BufStream::new(stream);
//! buf.read(&mut [0; 1024]).unwrap();
//! buf.write(&[0; 1024]).unwrap();
//! ```

use std::fmt;
use std::io::prelude::*;
use std::io::{self, BufReader, BufWriter};

const DEFAULT_BUF_SIZE: usize = 64 * 1024;

/// Wraps a Stream and buffers input and output to and from it.
///
/// It can be excessively inefficient to work directly with a `Read+Write`. For
/// example, every call to `read` or `write` on `TcpStream` results in a system
/// call. A `BufStream` keeps in memory buffers of data, making large,
/// infrequent calls to `read` and `write` on the underlying `Read+Write`.
///
/// The output buffer will be written out when this stream is dropped.
#[derive(Debug)]
pub struct BufStream<S: Write> {
    inner: BufReader<InternalBufWriter<S>>
}

/// An error returned by `into_inner` which combines an error that
/// happened while writing out the buffer, and the buffered writer object
/// which may be used to recover from the condition.
#[derive(Debug)]
pub struct IntoInnerError<W>(W, io::Error);

struct InternalBufWriter<W: Write>(Option<BufWriter<W>>);

impl<W: Write> InternalBufWriter<W> {
    fn get_ref(&self) -> &BufWriter<W> {
        let InternalBufWriter(ref w) = *self;
        w.as_ref().unwrap()
    }

    fn get_mut(&mut self) -> &mut BufWriter<W> {
        let InternalBufWriter(ref mut w) = *self;
        w.as_mut().unwrap()
    }
}

impl<W: Read + Write> Read for InternalBufWriter<W> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.get_mut().get_mut().read(buf)
    }
}

impl<W: Write + fmt::Debug> fmt::Debug for InternalBufWriter<W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.get_ref().fmt(f)
    }
}

impl<S: Read + Write> BufStream<S> {
    /// Creates a new buffered stream with explicitly listed capacities for the
    /// reader/writer buffer.
    pub fn with_capacities(reader_cap: usize, writer_cap: usize, inner: S)
                           -> BufStream<S> {
        let writer = BufWriter::with_capacity(writer_cap, inner);
        let internal_writer = InternalBufWriter(Some(writer));
        let reader = BufReader::with_capacity(reader_cap, internal_writer);
        BufStream { inner: reader }
    }

    /// Creates a new buffered stream with the default reader/writer buffer
    /// capacities.
    pub fn new(inner: S) -> BufStream<S> {
        BufStream::with_capacities(DEFAULT_BUF_SIZE, DEFAULT_BUF_SIZE, inner)
    }

    /// Gets a reference to the underlying stream.
    pub fn get_ref(&self) -> &S {
        self.inner.get_ref().get_ref().get_ref()
    }

    /// Gets a mutable reference to the underlying stream.
    ///
    /// # Warning
    ///
    /// It is inadvisable to read directly from or write directly to the
    /// underlying stream.
    pub fn get_mut(&mut self) -> &mut S {
        self.inner.get_mut().get_mut().get_mut()
    }

    /// Unwraps this `BufStream`, returning the underlying stream.
    ///
    /// The internal write buffer is written out before returning the stream.
    /// Any leftover data in the read buffer is lost.
    pub fn into_inner(mut self) -> Result<S, IntoInnerError<BufStream<S>>> {
        let e = {
            let InternalBufWriter(ref mut w) = *self.inner.get_mut();
            let (e, w2) = match w.take().unwrap().into_inner() {
                Ok(s) => return Ok(s),
                Err(err) => {
                    (io::Error::new(err.error().kind(), err.error().to_string()),
                     err.into_inner())
                }
            };
            *w = Some(w2);
            e
        };
        Err(IntoInnerError(self, e))
    }
}

impl<S: Read + Write> BufRead for BufStream<S> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> { self.inner.fill_buf() }
    fn consume(&mut self, amt: usize) { self.inner.consume(amt) }
}

impl<S: Read + Write> Read for BufStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<S: Read + Write> Write for BufStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.get_mut().0.as_mut().unwrap().get_mut().write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.get_mut().0.as_mut().unwrap().get_mut().flush()
    }
}

#[cfg(test)]
mod tests {
    use std::io::prelude::*;
    use std::io;

    use super::BufStream;
    // This is just here to make sure that we don't infinite loop in the
    // newtype struct autoderef weirdness
    #[test]
    fn test_buffered_stream() {
        struct S;

        impl Write for S {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> { Ok(b.len()) }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }

        impl Read for S {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { Ok(0) }
        }

        let mut stream = BufStream::new(S);
        assert_eq!(stream.read(&mut [0; 10]).unwrap(), 0);
        stream.write(&[0; 10]).unwrap();
        stream.flush().unwrap();
    }
}
