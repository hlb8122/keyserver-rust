pub trait Key
where
    Self: From<Vec<u8>> + Into<Vec<u8>>,
{
    fn get_address(&self) -> Vec<u8>;
}

pub trait Signature
where
    Self: From<Vec<u8>> + Into<Vec<u8>>,
{
}

pub trait SigScheme {
    type Key: Key;
    type Signature: Signature;

    fn verify(key: Self::Key, sig: Self::Signature) -> bool;
}
