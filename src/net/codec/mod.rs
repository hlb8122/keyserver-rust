pub mod generic;

use generic::{DecodeBuf, EncodeBuf};

use bytes::BufMut;
use prost::DecodeError;
use prost::Message;
use std::marker::PhantomData;

#[derive(Debug)]
pub struct Codec<T, U>(PhantomData<(T, U)>);

#[derive(Debug)]
pub struct Encoder<T>(PhantomData<T>);

#[derive(Debug)]
pub struct Decoder<T>(PhantomData<T>);

impl<T, U> Default for Codec<T, U>
where
    T: Message,
    U: Message + Default,
{
    fn default() -> Self {
        Codec(PhantomData)
    }
}

impl<T, U> generic::Codec for Codec<T, U>
where
    T: Message,
    U: Message + Default,
{
    type Encode = T;
    type Encoder = Encoder<T>;
    type Decode = U;
    type Decoder = Decoder<U>;

    fn encoder(&mut self) -> Self::Encoder {
        Encoder(PhantomData)
    }

    fn decoder(&mut self) -> Self::Decoder {
        Decoder(PhantomData)
    }
}

impl<T, U> Clone for Codec<T, U> {
    fn clone(&self) -> Self {
        Codec(PhantomData)
    }
}

impl<T> Default for Encoder<T>
where
    T: Message,
{
    fn default() -> Self {
        Encoder(PhantomData)
    }
}

impl<T> generic::Encoder for Encoder<T>
where
    T: Message,
{
    type Item = T;

    fn encode(&mut self, item: T, buf: &mut EncodeBuf<'_>) -> Result<(), generic::Status> {
        let len = item.encoded_len();

        if buf.remaining_mut() < len {
            buf.reserve(len);
        }

        item.encode(buf)
            .map_err(|_| unreachable!("Message only errors if not enough space"))
    }
}

impl<T> Clone for Encoder<T> {
    fn clone(&self) -> Self {
        Encoder(PhantomData)
    }
}

impl<T> Default for Decoder<T>
where
    T: Message + Default,
{
    fn default() -> Self {
        Decoder(PhantomData)
    }
}

fn from_decode_error(error: DecodeError) -> generic::Status {
    generic::Status::new(generic::Code::Internal, error.to_string())
}

impl<T> generic::Decoder for Decoder<T>
where
    T: Message + Default,
{
    type Item = T;

    fn decode(&mut self, buf: &mut DecodeBuf<'_>) -> Result<T, generic::Status> {
        Message::decode(buf).map_err(from_decode_error)
    }
}

impl<T> Clone for Decoder<T> {
    fn clone(&self) -> Self {
        Decoder(PhantomData)
    }
}
