use bytes::{Buf, BufMut, Bytes, BytesMut};

pub trait Codec {
    type Encode;
    type Encoder: Encoder<Item = Self::Encode>;
    type Decode;
    type Decoder: Decoder<Item = Self::Decode>;

    fn encoder(&mut self) -> Self::Encoder;
    fn decoder(&mut self) -> Self::Decoder;
}

pub trait Encoder {
    type Item;

    fn encode(&mut self, item: Self::Item, buf: &mut EncodeBuf<'_>) -> Result<(), Status>;
}

#[derive(Debug)]
pub struct EncodeBuf<'a> {
    bytes: &'a mut BytesMut,
}

pub trait Decoder {
    type Item;

    fn decode(&mut self, buf: &mut DecodeBuf<'_>) -> Result<Self::Item, Status>;
}

pub struct DecodeBuf<'a> {
    bufs: &'a mut dyn Buf,
    len: usize,
}

#[derive(Clone)]
pub struct Status {
    code: Code,
    message: String,
    details: Bytes,
}

impl Status {
    pub fn new(code: Code, message: impl Into<String>) -> Status {
        Status {
            code,
            message: message.into(),
            details: Bytes::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    Ok = 0,
    Cancelled = 1,
    Unknown = 2,
    InvalidArgument = 3,
    DeadlineExceeded = 4,
    NotFound = 5,
    AlreadyExists = 6,
    PermissionDenied = 7,
    ResourceExhausted = 8,
    FailedPrecondition = 9,
    Aborted = 10,
    OutOfRange = 11,
    Unimplemented = 12,
    Internal = 13,
    Unavailable = 14,
    DataLoss = 15,
    Unauthenticated = 16,
    __NonExhaustive,
}

impl<'a> EncodeBuf<'a> {
    #[inline]
    pub fn reserve(&mut self, capacity: usize) {
        self.bytes.reserve(capacity);
    }
}

impl<'a> BufMut for EncodeBuf<'a> {
    #[inline]
    fn remaining_mut(&self) -> usize {
        self.bytes.remaining_mut()
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        self.bytes.advance_mut(cnt)
    }

    #[inline]
    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        self.bytes.bytes_mut()
    }
}

impl<'a> Buf for DecodeBuf<'a> {
    #[inline]
    fn remaining(&self) -> usize {
        self.len
    }

    #[inline]
    fn bytes(&self) -> &[u8] {
        let ret = self.bufs.bytes();

        if ret.len() > self.len {
            &ret[..self.len]
        } else {
            ret
        }
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.len);
        self.bufs.advance(cnt);
        self.len -= cnt;
    }
}
