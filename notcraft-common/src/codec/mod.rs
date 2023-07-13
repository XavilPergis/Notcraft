//! This module provides facilities for encoding and decoding a custom binary
//! save format.
//!
//! # Format Description
//!
//! ```no_run
//! def NODE_TYPE_NODE     = 0:u8
//! def NODE_TYPE_LIST     = 1:u8
//! def NODE_TYPE_MAP      = 2:u8
//! def NODE_TYPE_RAW      = 3:u8
//! def NODE_TYPE_STRING   = 4:u8
//! def NODE_TYPE_BOOL     = 5:u8
//! def NODE_TYPE_UNSIGNED = 6:u8
//! def NODE_TYPE_SIGNED   = 7:u8
//! def NODE_TYPE_F32      = 8:u8
//! def NODE_TYPE_F64      = 9:u8
//!
//! def unsingedVarInt = /* variable-length quantity */
//! def singedVarInt   = /* variable-length quantity */
//!
//! // line a byteSequence, but has the additional constraint of being valid UTF-8
//! def string       = length:unsingedVarInt ~ data:{u8{length}}
//! def byteSequence = length:unsingedVarInt ~ data:{u8{length}}
//!
//! def root =
//!     ~ formatVersion:u64be
//!     ~ rootNode:mapNode
//!
//! def node =
//!     / NODE_TYPE_NODE     ~ node
//!     / NODE_TYPE_MAP      ~ mapNode
//!     / NODE_TYPE_LIST     ~ listNode
//!     / NODE_TYPE_RAW      ~ byteSequence
//!     / NODE_TYPE_STRING   ~ string
//!     / NODE_TYPE_BOOL     ~ bool
//!     / NODE_TYPE_UNSIGNED ~ unsingedVarInt
//!     / NODE_TYPE_SIGNED   ~ singedVarInt
//!     / NODE_TYPE_F32      ~ f32be
//!     / NODE_TYPE_F64      ~ f64be
//!
//! def LIST_TYPE_VERBATIM = 0:u8
//! def LIST_TYPE_RLE      = 1:u8
//!
//! // lists are homogenous, though you can store a NODE_TYPE_NODE to make the list effectively heterogeneous.
//! def listNode =
//!     / LIST_TYPE_VERBATIM ~ verbatimListNode
//!     / LIST_TYPE_RLE      ~ rleListNode
//!
//! def rleListNode =
//!     / NODE_TYPE_NODE     ~ { !0:u8 ~ runLength:unsingedVarInt ~ node           }* ~ 0:u8
//!     / NODE_TYPE_MAP      ~ { !0:u8 ~ runLength:unsignedVarInt ~ mapNode        }* ~ 0:u8
//!     / NODE_TYPE_LIST     ~ { !0:u8 ~ runLength:unsignedVarInt ~ listNode       }* ~ 0:u8
//!     / NODE_TYPE_RAW      ~ { !0:u8 ~ runLength:unsignedVarInt ~ byteSequence   }* ~ 0:u8
//!     / NODE_TYPE_STRING   ~ { !0:u8 ~ runLength:unsignedVarInt ~ string         }* ~ 0:u8
//!     / NODE_TYPE_BOOL     ~ { !0:u8 ~ runLength:unsignedVarInt ~ bool           }* ~ 0:u8
//!     / NODE_TYPE_UNSIGNED ~ { !0:u8 ~ runLength:unsignedVarInt ~ unsingedVarInt }* ~ 0:u8
//!     / NODE_TYPE_SIGNED   ~ { !0:u8 ~ runLength:unsignedVarInt ~ singedVarInt   }* ~ 0:u8
//!     / NODE_TYPE_F32      ~ { !0:u8 ~ runLength:unsignedVarInt ~ f32be          }* ~ 0:u8
//!     / NODE_TYPE_F64      ~ { !0:u8 ~ runLength:unsignedVarInt ~ f64be          }* ~ 0:u8
//!
//! def verbatimListNode = length:unsingedVarInt ~ {
//!     / NODE_TYPE_NODE     ~ node{length}
//!     / NODE_TYPE_MAP      ~ mapNode{length}
//!     / NODE_TYPE_LIST     ~ listNode{length}
//!     / NODE_TYPE_RAW      ~ byteSequence{length}
//!     / NODE_TYPE_STRING   ~ string{length}
//!     / NODE_TYPE_BOOL     ~ bool{length}
//!     / NODE_TYPE_UNSIGNED ~ unsingedVarInt{length}
//!     / NODE_TYPE_SIGNED   ~ singedVarInt{length}
//!     / NODE_TYPE_F32      ~ f32be{length}
//!     / NODE_TYPE_F64      ~ f64be{length}
//! }
//!
//! def mapNode = { !0:u8 ~ key:string ~ value:node }* ~ 0:u8
//! ```

pub mod decode;
pub mod encode;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum NodeKind {
    /// "wrapper" node which only contains another node.
    Node = 0,
    Map = 1,
    List = 2,
    /// raw binary data
    Raw = 3,
    /// UTF-8 encoded string
    String = 4,
    Bool = 5,

    // VLQ-encoded integers
    UnsignedVarInt = 6,
    SignedVarInt = 7,

    // floating-point numbers
    Float32 = 8,
    Float64 = 9,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum ListKind {
    Verbatim = 0,
    RunLength = 1,
}
