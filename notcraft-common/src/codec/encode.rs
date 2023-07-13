use super::{ListKind, NodeKind};
use crate::prelude::*;
use std::io::Write;

trait BaseEncode<W> {
    fn encode(&self, writer: &mut W) -> Result<()>;
}

pub trait Encode<W> {
    const KIND: NodeKind;
    fn encode(&self, encoder: Encoder<W>) -> Result<()>;
}

pub struct MapEncoder<'w, W> {
    writer: &'w mut W,
}

impl<'w, W: Write> MapEncoder<'w, W> {
    // see `mapNode` in module-level documentation for format specification
    pub fn entry<'e, 'k>(&'e mut self, key: &'k str) -> MapEncoderEntry<'w, 'e, 'k, W> {
        assert!(!key.is_empty(), "tried encoding map entry with empty key");
        MapEncoderEntry { encoder: self, key }
    }
}

pub struct MapEncoderEntry<'w, 'e, 's, W> {
    encoder: &'e mut MapEncoder<'w, W>,
    key: &'s str,
}

impl<'w, W: Write> MapEncoderEntry<'w, '_, '_, W> {
    fn encode_header(&mut self, kind: NodeKind) -> Result<()> {
        encode_base(self.encoder.writer, self.key)?;
        encode_base(self.encoder.writer, &FixedInt(kind as u8))?;
        Ok(())
    }

    // see `mapNode` in module-level documentation for format specification
    pub fn encode<T: Encode<W>>(mut self, item: &T) -> Result<()> {
        self.encode_header(<T as Encode<W>>::KIND)?;
        encode(self.encoder.writer, item)?;
        Ok(())
    }

    pub fn encode_verbatim_list<I>(mut self, iter: I) -> Result<()>
    where
        I: ExactSizeIterator,
        I::Item: Encode<W>,
    {
        self.encode_header(NodeKind::List)?;
        encode_verbatim_list(self.encoder.writer, iter)?;
        Ok(())
    }

    pub fn encode_rle_list<I>(mut self, iter: I) -> Result<()>
    where
        I: Iterator,
        I::Item: Encode<W> + PartialEq,
    {
        self.encode_header(NodeKind::List)?;
        encode_rle_list(self.encoder.writer, iter)?;
        Ok(())
    }

    pub fn encode_rle_list_runs<T, I>(mut self, iter: I) -> Result<()>
    where
        I: Iterator<Item = (usize, T)>,
        T: Encode<W> + PartialEq,
    {
        self.encode_header(NodeKind::List)?;
        encode_rle_list_runs(self.encoder.writer, iter)?;
        Ok(())
    }

    pub fn encode_map<F>(mut self, func: F) -> Result<()>
    where
        F: FnOnce(MapEncoder<'_, W>) -> Result<()>,
    {
        self.encode_header(NodeKind::Map)?;
        encode_map(self.encoder.writer, func)?;
        Ok(())
    }
}

// TODO: currently, there is no verification that you write the same kind of
// node that's specified in [`Encode::KIND`]
pub struct Encoder<'w, W> {
    writer: &'w mut W,
}

impl<'w, W: Write> Encoder<'w, W> {
    pub fn encode<T: Encode<W>>(self, item: &T) -> Result<()> {
        encode(self.writer, item)
    }

    pub fn encode_verbatim_list<I>(self, iter: I) -> Result<()>
    where
        I: ExactSizeIterator,
        I::Item: Encode<W>,
    {
        encode_verbatim_list(self.writer, iter)
    }

    pub fn encode_rle_list<I>(self, iter: I) -> Result<()>
    where
        I: Iterator,
        I::Item: Encode<W> + PartialEq,
    {
        encode_rle_list(self.writer, iter)
    }

    pub fn encode_rle_list_runs<T, I>(self, iter: I) -> Result<()>
    where
        I: Iterator<Item = (usize, T)>,
        T: Encode<W> + PartialEq,
    {
        encode_rle_list_runs(self.writer, iter)
    }

    pub fn encode_map<F>(self, func: F) -> Result<()>
    where
        F: FnOnce(MapEncoder<'_, W>) -> Result<()>,
    {
        encode_map(self.writer, func)
    }
}

fn encode_base<W, T: BaseEncode<W> + ?Sized>(writer: &mut W, item: &T) -> Result<()> {
    T::encode(item, writer)
}

fn encode<W, T: Encode<W> + ?Sized>(writer: &mut W, item: &T) -> Result<()> {
    T::encode(item, Encoder { writer })
}

fn encode_verbatim_list<W, I>(writer: &mut W, mut iter: I) -> Result<()>
where
    I: ExactSizeIterator,
    I::Item: Encode<W>,
    W: Write,
{
    // see `verbatimListNode` in module-level documentation for format specification
    encode_base(writer, &FixedInt(ListKind::Verbatim as u8))?;
    encode_base(writer, &VarInt(iter.len() as u32))?;
    encode_base(writer, &FixedInt(<I::Item as Encode<W>>::KIND as u8))?;

    for item in iter.by_ref() {
        encode(writer, &item)?;
    }

    Ok(())
}

fn encode_rle_list<W, I>(writer: &mut W, mut iter: I) -> Result<()>
where
    I: Iterator,
    I::Item: Encode<W> + PartialEq,
    W: Write,
{
    // see `rleListNode` in module-level documentation for format specification
    encode_base(writer, &FixedInt(ListKind::RunLength as u8))?;
    encode_base(writer, &FixedInt(<I::Item as Encode<W>>::KIND as u8))?;

    if let Some(first_item) = iter.next() {
        let mut current_run_len = 1usize;
        let mut current_run_element = first_item;

        for element in iter {
            if current_run_element != element {
                encode_base(writer, &VarInt(current_run_len))?;
                encode(writer, &current_run_element)?;
                current_run_len = 1;
                current_run_element = element;
            } else {
                current_run_len += 1;
            }
        }

        encode_base(writer, &VarInt(current_run_len))?;
        encode(writer, &current_run_element)?;
    }

    encode_base(writer, &VarInt(0u8))?;

    Ok(())
}

fn encode_rle_list_runs<W, T, I>(writer: &mut W, mut iter: I) -> Result<()>
where
    I: Iterator<Item = (usize, T)>,
    T: Encode<W> + PartialEq,
    W: Write,
{
    encode_base(writer, &FixedInt(ListKind::RunLength as u8))?;

    // see `rleListNode` in module-level documentation for format specification
    encode_base(writer, &FixedInt(<T as Encode<W>>::KIND as u8))?;

    while let Some((len, item)) = iter.next() {
        encode_base(writer, &VarInt(len))?;
        encode(writer, &item)?;
    }

    encode_base(writer, &VarInt(0u8))?;

    Ok(())
}

fn encode_map<W, F>(writer: &mut W, func: F) -> Result<()>
where
    F: FnOnce(MapEncoder<'_, W>) -> Result<()>,
    W: Write,
{
    // see `mapNode` in module-level documentation for format specification
    func(MapEncoder { writer })?;
    encode_base(writer, &VarInt(0u8))?;
    Ok(())
}

impl<W, T: Encode<W>> Encode<W> for &'_ T {
    const KIND: NodeKind = <T as Encode<W>>::KIND;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        <T as Encode<W>>::encode(&**self, encoder)
    }
}

impl<W, T: Encode<W>> Encode<W> for &'_ mut T {
    const KIND: NodeKind = <T as Encode<W>>::KIND;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        <T as Encode<W>>::encode(&**self, encoder)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct Verbatim<T>(pub T);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct RunLength<T>(pub T);

impl<W: std::io::Write, T: Encode<W>> Encode<W> for Verbatim<Vec<T>> {
    const KIND: NodeKind = NodeKind::List;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        encoder.encode_verbatim_list(self.0.iter())
    }
}

impl<W: std::io::Write, T: Encode<W> + PartialEq> Encode<W> for RunLength<Vec<T>> {
    const KIND: NodeKind = NodeKind::List;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        encoder.encode_rle_list(self.0.iter())
    }
}

impl<'a, W: std::io::Write, T: Encode<W>> Encode<W> for Verbatim<&'a [T]> {
    const KIND: NodeKind = NodeKind::List;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        encoder.encode_verbatim_list(self.0.iter())
    }
}

impl<'a, W: std::io::Write, T: Encode<W> + PartialEq> Encode<W> for RunLength<&'a [T]> {
    const KIND: NodeKind = NodeKind::List;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        encoder.encode_rle_list(self.0.iter())
    }
}

impl<W: Write> BaseEncode<W> for str {
    fn encode(&self, writer: &mut W) -> Result<()> {
        <_ as BaseEncode<W>>::encode(&VarInt(self.len()), writer)?;
        writer.write_all(self.as_bytes())?;
        Ok(())
    }
}

impl<W: Write> Encode<W> for str {
    const KIND: NodeKind = NodeKind::String;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        <Self as BaseEncode<W>>::encode(&self, encoder.writer)
    }
}

impl<W: Write> BaseEncode<W> for bool {
    fn encode(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&[*self as u8])?;
        Ok(())
    }
}

impl<W: Write> Encode<W> for bool {
    const KIND: NodeKind = NodeKind::Bool;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        <Self as BaseEncode<W>>::encode(&self, encoder.writer)
    }
}

impl<W: Write> BaseEncode<W> for f32 {
    fn encode(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_be_bytes())?;
        Ok(())
    }
}

impl<W: Write> Encode<W> for f32 {
    const KIND: NodeKind = NodeKind::Float32;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        <Self as BaseEncode<W>>::encode(&self, encoder.writer)
    }
}

impl<W: Write> BaseEncode<W> for f64 {
    fn encode(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_be_bytes())?;
        Ok(())
    }
}

impl<W: Write> Encode<W> for f64 {
    const KIND: NodeKind = NodeKind::Float64;

    fn encode(&self, encoder: Encoder<W>) -> Result<()> {
        <Self as BaseEncode<W>>::encode(&self, encoder.writer)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
#[repr(transparent)]
pub struct FixedInt<T>(pub T);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct VarInt<T>(pub T);

macro_rules! impl_fixed_numeric_encode {
    ($($type:ty,)*) => {
        $(impl<W: Write> BaseEncode<W> for FixedInt<$type> {
            fn encode(&self, writer: &mut W) -> Result<()> {
                writer.write_all(&self.0.to_be_bytes())?;
                Ok(())
            }
        })*
    };
}

impl_fixed_numeric_encode! { u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, }

macro_rules! impl_numeric_encode {
    (@encode $kind:expr, $($type:ty,)*) => {
        $(impl<W: Write> Encode<W> for $type {
            const KIND: NodeKind = $kind;

            fn encode(&self, encoder: Encoder<W>) -> Result<()> {
                <VarInt<$type> as BaseEncode<W>>::encode(&VarInt(*self), encoder.writer)
            }
        })*
    };

    (signed { $($type:ty,)* }; $($rest:tt)*) => {
        $(impl<W: Write> BaseEncode<W> for VarInt<$type> {
            fn encode(&self, writer: &mut W) -> Result<()> {
                let VarInt(value) = *self;
                let (sign, unsigned) = match value >= 0 {
                    true => (0, value as u8),
                    false => (0x40, value.abs() as u8),
                };

                let mut cur = unsigned;
                while cur > 0x3f {
                    writer.write_all(&[0x80 | (cur & 0x7f) as u8])?;
                    cur >>= 7;
                }
                writer.write_all(&[sign | cur as u8])?;

                Ok(())
            }
        })*

        impl_numeric_encode! { @encode NodeKind::SignedVarInt, $($type,)* }
        impl_numeric_encode! { $($rest)* }
    };

    (unsigned { $($type:ty,)* }; $($rest:tt)*) => {
        $(impl<W: Write> BaseEncode<W> for VarInt<$type> {
            fn encode(&self, writer: &mut W) -> Result<()> {
                let VarInt(mut cur) = *self;
                while cur > 0x7f {
                    writer.write_all(&[0x80 | (cur & 0x7f) as u8])?;
                    cur >>= 7;
                }
                writer.write_all(&[cur as u8])?;

                Ok(())
            }
        })*

        impl_numeric_encode! { @encode NodeKind::UnsignedVarInt, $($type,)* }
        impl_numeric_encode! { $($rest)* }
    };

    () => {};
}

impl_numeric_encode! {
    signed { i8, i16, i32, i64, i128, isize, };
    unsigned { u8, u16, u32, u64, u128, usize, };
}
