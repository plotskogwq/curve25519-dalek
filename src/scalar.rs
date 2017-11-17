// -*- mode: rust; -*-
//
// This file is part of curve25519-dalek.
// Copyright (c) 2016-2017 Isis Lovecruft, Henry de Valence
// Portions Copyright 2017 Brian Smith
// See LICENSE for licensing information.
//
// Authors:
// - Isis Agora Lovecruft <isis@patternsinthevoid.net>
// - Henry de Valence <hdevalence@hdevalence.ca>
// - Brian Smith <brian@briansmith.org>

//! Arithmetic for scalar multiplication.
//!
//! The Ed25519 basepoint P has prime order
//!
//! l = 2^252 + 27742317777372353535851937790883648493.
//!
//! Thus a multiple `aP` of the basepoint (with a ∈ ℤ) depends only
//! on the value of `a (mod l)`, or equivalently, the image of `a` in
//! the quotient ℤ/lℤ.
//!
//! The `Scalar` struct represents an element in ℤ/lℤ.
//!
//! In contrast to `FieldElement`s, `Scalar`s are stored in
//! memory as bytes, allowing easy access to the bits of the `Scalar`
//! when multiplying a point by a scalar.  For efficient arithmetic
//! between two scalars, the `UnpackedScalar` struct (internally
//! either `Scalar32` or `Scalar64`) is stored as limbs.

use core::fmt::Debug;
use core::ops::Neg;
use core::ops::{Add, AddAssign};
use core::ops::{Sub, SubAssign};
use core::ops::{Mul, MulAssign};
use core::ops::{Index, IndexMut};
use core::cmp::{Eq, PartialEq};

#[cfg(feature = "std")]
use rand::Rng;

use digest::Digest;
use generic_array::typenum::U64;

use subtle::slices_equal;
use subtle::ConditionallyAssignable;
use subtle::Equal;

use backend;

/// An `UnpackedScalar` represents an element of the field GF(l), optimized for speed.
#[cfg(feature="radix_51")]
type UnpackedScalar = backend::u64::scalar::Scalar64;

/// An `UnpackedScalar` represents an element of the field GF(l), optimized for speed.
#[cfg(not(feature="radix_51"))]
type UnpackedScalar = backend::u32::scalar::Scalar32;


/// The `Scalar` struct represents an element in ℤ/lℤ, where
///
/// l = 2^252 + 27742317777372353535851937790883648493
///
/// is the order of the basepoint.  The `Scalar` is stored as bytes.
#[derive(Copy, Clone)]
pub struct Scalar(pub [u8; 32]);

impl Debug for Scalar {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        write!(f, "Scalar: {:?}", &self.0[..])
    }
}

impl Eq for Scalar {}
impl PartialEq for Scalar {
    /// Test equality between two `Scalar`s.
    ///
    /// # Warning
    ///
    /// This function is *not* guaranteed to be constant time and should only be
    /// used for debugging purposes.
    ///
    /// # Returns
    ///
    /// True if they are equal, and false otherwise.
    fn eq(&self, other: &Self) -> bool {
        slices_equal(&self.0, &other.0) == 1u8
    }
}

impl Equal for Scalar {
    /// Test equality between two `Scalar`s in constant time.
    ///
    /// # Returns
    ///
    /// `1u8` if they are equal, and `0u8` otherwise.
    fn ct_eq(&self, other: &Self) -> u8 {
        slices_equal(&self.0, &other.0)
    }
}

impl Index<usize> for Scalar {
    type Output = u8;

    fn index(&self, _index: usize) -> &u8 {
        &(self.0[_index])
    }
}

impl IndexMut<usize> for Scalar {
    fn index_mut(&mut self, _index: usize) -> &mut u8 {
        &mut (self.0[_index])
    }
}

impl<'b> MulAssign<&'b Scalar> for Scalar {
    fn mul_assign(&mut self, _rhs: &'b Scalar) {
        *self = Scalar::mul(self, _rhs)
    }
}

impl<'a, 'b> Mul<&'b Scalar> for &'a Scalar {
    type Output = Scalar;
    fn mul(self, _rhs: &'b Scalar) -> Scalar {
        Scalar::mul(self, _rhs)
    }
}

impl<'b> AddAssign<&'b Scalar> for Scalar {
    fn add_assign(&mut self, _rhs: &'b Scalar) {
        *self = Scalar::add(self, _rhs);
    }
}

impl<'a, 'b> Add<&'b Scalar> for &'a Scalar {
    type Output = Scalar;
    fn add(self, _rhs: &'b Scalar) -> Scalar {
        Scalar::add(self, _rhs)
    }
}

impl<'b> SubAssign<&'b Scalar> for Scalar {
    fn sub_assign(&mut self, _rhs: &'b Scalar) {
        *self = Scalar::sub(self, _rhs);
    }
}

impl<'a, 'b> Sub<&'b Scalar> for &'a Scalar {
    type Output = Scalar;
    fn sub(self, _rhs: &'b Scalar) -> Scalar {
        Scalar::sub(self, _rhs)
    }
}

impl<'a> Neg for &'a Scalar {
    type Output = Scalar;
    fn neg(self) -> Scalar {
        Scalar::sub(&Scalar::zero(), self)
    }
}

impl ConditionallyAssignable for Scalar {
    /// Conditionally assign another Scalar to this one.
    ///
    /// ```
    /// # extern crate curve25519_dalek;
    /// # extern crate subtle;
    /// # use curve25519_dalek::scalar::Scalar;
    /// # use subtle::ConditionallyAssignable;
    /// # fn main() {
    /// let a = Scalar([0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
    ///                 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]);
    /// let b = Scalar([1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,
    ///                 1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1]);
    /// let mut t = a;
    /// t.conditional_assign(&b, 0u8);
    /// assert!(t[0] == a[0]);
    /// t.conditional_assign(&b, 1u8);
    /// assert!(t[0] == b[0]);
    /// # }
    /// ```
    ///
    /// # Preconditions
    ///
    /// * `choice` in {0,1}
    // XXX above test checks first byte because Scalar does not impl Eq
    fn conditional_assign(&mut self, other: &Scalar, choice: u8) {
        // if choice = 0u8, mask = (-0i8) as u8 = 00000000
        // if choice = 1u8, mask = (-1i8) as u8 = 11111111
        let mask = -(choice as i8) as u8;
        for i in 0..32 {
            self[i] ^= mask & (self[i] ^ other[i]);
        }
    }
}

#[cfg(feature = "serde")]
use serde::{self, Serialize, Deserialize, Serializer, Deserializer};
#[cfg(feature = "serde")]
use serde::de::Visitor;

#[cfg(feature = "serde")]
impl Serialize for Scalar {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_bytes(self.as_bytes())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Scalar {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        struct ScalarVisitor;

        impl<'de> Visitor<'de> for ScalarVisitor {
            type Value = Scalar;

            fn expecting(&self, formatter: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                formatter.write_str("a 32-byte scalar value")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Scalar, E>
                where E: serde::de::Error
            {
                if v.len() == 32 {
                    // array_ref turns &[u8] into &[u8;32]
                    Ok(Scalar(*array_ref!(v, 0, 32)))
                } else {
                    Err(serde::de::Error::invalid_length(v.len(), &self))
                }
            }
        }

        deserializer.deserialize_bytes(ScalarVisitor)
    }
}

impl Scalar {
    /// Return a `Scalar` chosen uniformly at random using a user-provided RNG.
    ///
    /// # Inputs
    ///
    /// * `rng`: any RNG which implements the `rand::Rng` interface.
    ///
    /// # Returns
    ///
    /// A random scalar within ℤ/lℤ.
    #[cfg(feature = "std")]
    pub fn random<T: Rng>(rng: &mut T) -> Self {
        let mut scalar_bytes = [0u8; 64];
        rng.fill_bytes(&mut scalar_bytes);
        Scalar::reduce(&scalar_bytes)
    }

    /// Hash a slice of bytes into a scalar.
    ///
    /// Takes a type parameter `D`, which is any `Digest` producing 64
    /// bytes (512 bits) of output.
    ///
    /// Convenience wrapper around `from_hash`.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate curve25519_dalek;
    /// # use curve25519_dalek::scalar::Scalar;
    /// extern crate sha2;
    /// use sha2::Sha512;
    ///
    /// # // Need fn main() here in comment so the doctest compiles
    /// # // See https://doc.rust-lang.org/book/documentation.html#documentation-as-tests
    /// # fn main() {
    /// let msg = "To really appreciate architecture, you may even need to commit a murder";
    /// let s = Scalar::hash_from_bytes::<Sha512>(msg.as_bytes());
    /// # }
    /// ```
    ///
    pub fn hash_from_bytes<D>(input: &[u8]) -> Scalar
        where D: Digest<OutputSize = U64> + Default
    {
        let mut hash = D::default();
        hash.input(input);
        Scalar::from_hash(hash)
    }

    /// Construct a scalar from an existing `Digest` instance.
    ///
    /// Use this instead of `hash_from_bytes` if it is more convenient
    /// to stream data into the `Digest` than to pass a single byte
    /// slice.
    pub fn from_hash<D>(hash: D) -> Scalar
        where D: Digest<OutputSize = U64> + Default
    {
        // XXX this seems clumsy
        let mut output = [0u8; 64];
        output.copy_from_slice(hash.result().as_slice());
        Scalar::reduce(&output)
    }

    /// View this `Scalar` as a sequence of bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Construct the additive identity
    pub fn zero() -> Self {
        Scalar([0u8; 32])
    }

    /// Construct the multiplicative identity
    pub fn one() -> Self {
        Scalar([ 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ])
    }

    /// Construct a scalar from the given `u64`.
    pub fn from_u64(x: u64) -> Scalar {
        let mut s = Scalar::zero();
        for i in 0..8 {
            s[i] = (x >> (i*8)) as u8;
        }
        s
    }

    /// Compute the multiplicative inverse of this scalar.
    pub fn invert(&self) -> Scalar {
        self.unpack().invert().pack()
    }

    /// Get the bits of the scalar.
    pub fn bits(&self) -> [i8; 256] {
        let mut bits = [0i8; 256];
        for i in 0..256 {
            // As i runs from 0..256, the bottom 3 bits index the bit,
            // while the upper bits index the byte.
            bits[i] = ((self.0[i>>3] >> (i&7)) & 1u8) as i8;
        }
        bits
    }

    /// Compute a width-5 "Non-Adjacent Form" of this scalar.
    ///
    /// A width-`w` NAF of a positive integer `k` is an expression
    /// `k = sum(k[i]*2^i for i in range(l))`, where each nonzero
    /// coefficient `k[i]` is odd and bounded by `|k[i]| < 2^(w-1)`,
    /// `k[l-1]` is nonzero, and at most one of any `w` consecutive
    /// coefficients is nonzero.  (Hankerson, Menezes, Vanstone; def 3.32).
    ///
    /// Intuitively, this is like a binary expansion, except that we
    /// allow some coefficients to grow up to `2^(w-1)` so that the
    /// nonzero coefficients are as sparse as possible.
    pub fn non_adjacent_form(&self) -> [i8; 256] {
        // Step 1: write out bits of the scalar
        let mut naf = self.bits();

        // Step 2: zero coefficients by carrying them upwards or downwards 
        'bits: for i in 0..256 {
            if naf[i] == 0 { continue 'bits; }
            'window: for b in 1..6 {
                if     i+b  >= 256  { break 'window; }
                if naf[i+b] == 0    { continue 'window; }
                let potential_carry = naf[i+b] << b;
                if  naf[i+b] + potential_carry <= 15 {
                    // Eliminate naf[i+b] by carrying its value onto naf[i]
                    naf[i] += potential_carry;
                    naf[i+b] = 0;
                } else if naf[i+b] - potential_carry >= -15 {
                    // Eliminate naf[i+b] by carrying its value upwards.
                    naf[i] -= potential_carry; // Subtract 2^(i+b)
                    'carry: for k in i+b..256 {
                        if naf[k] != 0 {
                            // Since naf[k] = 0 or 1 for k > i, naf[k] == 1.
                            naf[k] = 0; // Subtract 2^k
                        } else {
                            // By now we have subtracted 2^k = 
                            // 2^(i+b) + 2^(i+b) + 2^(i+b+1) + ... + 2^(k-1).
                            naf[k] = 1; // Add back 2^k.
                            break 'carry;
                        }
                    }
                }
            }
        }

        naf
    }

    /// Write this scalar in radix 16, with coefficients in `[-8,8)`,
    /// i.e., compute `a_i` such that
    ///
    ///    a = a_0 + a_1*16^1 + ... + a_63*16^63,
    ///
    /// with `-8 ≤ a_i < 8` for `0 ≤ i < 63` and `-8 ≤ a_63 ≤ 8`.
    ///
    /// Precondition: self[31] <= 127.  This is the case whenever
    /// `self` is reduced.
    pub fn to_radix_16(&self) -> [i8; 64] {
        debug_assert!(self[31] <= 127);
        let mut output = [0i8; 64];

        // Step 1: change radix.
        // Convert from radix 256 (bytes) to radix 16 (nibbles)
        #[inline(always)]
        fn bot_half(x: u8) -> u8 { (x >> 0) & 15 }
        #[inline(always)]
        fn top_half(x: u8) -> u8 { (x >> 4) & 15 }

        for i in 0..32 {
            output[2*i  ] = bot_half(self[i]) as i8;
            output[2*i+1] = top_half(self[i]) as i8;
        }
        // Precondition note: since self[31] <= 127, output[63] <= 7

        // Step 2: recenter coefficients from [0,16) to [-8,8)
        for i in 0..63 {
            let carry    = (output[i] + 8) >> 4;
            output[i  ] -= carry << 4;
            output[i+1] += carry;
        }
        // Precondition note: output[63] is not recentered.  It
        // increases by carry <= 1.  Thus output[63] <= 8.

        output
    }

    /// Unpack this `Scalar` to an `UnpackedScalar`
    pub(crate) fn unpack(&self) -> UnpackedScalar {
        UnpackedScalar::from_bytes(&self.0)
    }

    /// Compute `a + b` (mod l)
    pub fn add(a: &Scalar, b: &Scalar) -> Scalar {
        UnpackedScalar::add(&a.unpack(), &b.unpack()).pack()
    }

    /// Compute `a - b` (mod l).
    pub fn sub(a: &Scalar, b: &Scalar) -> Scalar {
        UnpackedScalar::sub(&a.unpack(), &b.unpack()).pack()
    }

    /// Compute `a * b` (mod l).
    pub fn mul(a: &Scalar, b: &Scalar) -> Scalar {
        UnpackedScalar::mul(&a.unpack(), &b.unpack()).pack()
    }

    /// Compute `(a * b) + c` (mod l).
    pub fn multiply_add(a: &Scalar, b: &Scalar, c: &Scalar) -> Scalar {
        UnpackedScalar::add(&UnpackedScalar::mul(&a.unpack(), &b.unpack()), &c.unpack()).pack()
    }

    /// Reduce a 512-bit little endian number mod l
    pub fn reduce(input: &[u8; 64]) -> Scalar {
        UnpackedScalar::from_bytes_wide(input).pack()
    }
}

impl UnpackedScalar {
    /// Pack the limbs of this `UnpackedScalar` into a `Scalar`.
    fn pack(&self) -> Scalar {
        Scalar(self.to_bytes())
    }

    /// Compute the multiplicative inverse of this scalar.
    pub fn invert(&self) -> UnpackedScalar {
        // This is a direct transliteration of the addition chain from
        // https://briansmith.org/ecc-inversion-addition-chains-01#curve25519_scalar_inversion
        // as it was published on 2017-09-03.

        let    _1 = self.to_montgomery();
        let   _10 = _1.montgomery_square();
        let  _100 = _10.montgomery_square();
        let   _11 = UnpackedScalar::montgomery_mul(&_10,     &_1);
        let  _101 = UnpackedScalar::montgomery_mul(&_10,    &_11);
        let  _111 = UnpackedScalar::montgomery_mul(&_10,   &_101);
        let _1001 = UnpackedScalar::montgomery_mul(&_10,   &_111);
        let _1011 = UnpackedScalar::montgomery_mul(&_10,  &_1001);
        let _1111 = UnpackedScalar::montgomery_mul(&_100, &_1011);

        // _10000
        let mut y = UnpackedScalar::montgomery_mul(&_1111, &_1);

        #[inline]
        fn square_multiply(y: &mut UnpackedScalar, squarings: usize, x: &UnpackedScalar) {
            for _ in 0..squarings {
                *y = y.montgomery_square();
            }
            *y = UnpackedScalar::montgomery_mul(y, x);
        }

        square_multiply(&mut y, 123 + 3, &_101);
        square_multiply(&mut y,   2 + 2, &_11);
        square_multiply(&mut y,   1 + 4, &_1111);
        square_multiply(&mut y,   1 + 4, &_1111);
        square_multiply(&mut y,       4, &_1001);
        square_multiply(&mut y,       2, &_11);
        square_multiply(&mut y,   1 + 4, &_1111);
        square_multiply(&mut y,   1 + 3, &_101);
        square_multiply(&mut y,   3 + 3, &_101);
        square_multiply(&mut y,       3, &_111);
        square_multiply(&mut y,   1 + 4, &_1111);
        square_multiply(&mut y,   2 + 3, &_111);
        square_multiply(&mut y,   2 + 2, &_11);
        square_multiply(&mut y,   1 + 4, &_1011);
        square_multiply(&mut y,   2 + 4, &_1011);
        square_multiply(&mut y,   6 + 4, &_1001);
        square_multiply(&mut y,   2 + 2, &_11);
        square_multiply(&mut y,   3 + 2, &_11);
        square_multiply(&mut y,   3 + 2, &_11);
        square_multiply(&mut y,   1 + 4, &_1001);
        square_multiply(&mut y,   1 + 3, &_111);
        square_multiply(&mut y,   2 + 4, &_1111);
        square_multiply(&mut y,   1 + 4, &_1011);
        square_multiply(&mut y,       3, &_101);
        square_multiply(&mut y,   2 + 4, &_1111);
        square_multiply(&mut y,       3, &_101);
        square_multiply(&mut y,   1 + 2, &_11);

        y.from_montgomery()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use constants;

    /// x = 2238329342913194256032495932344128051776374960164957527413114840482143558222
    pub static X: Scalar = Scalar(
        [0x4e, 0x5a, 0xb4, 0x34, 0x5d, 0x47, 0x08, 0x84,
         0x59, 0x13, 0xb4, 0x64, 0x1b, 0xc2, 0x7d, 0x52,
         0x52, 0xa5, 0x85, 0x10, 0x1b, 0xcc, 0x42, 0x44,
         0xd4, 0x49, 0xf4, 0xa8, 0x79, 0xd9, 0xf2, 0x04]);
    /// 1/x = 6859937278830797291664592131120606308688036382723378951768035303146619657244
    pub static XINV: Scalar = Scalar(
        [0x1c, 0xdc, 0x17, 0xfc, 0xe0, 0xe9, 0xa5, 0xbb,
         0xd9, 0x24, 0x7e, 0x56, 0xbb, 0x01, 0x63, 0x47,
         0xbb, 0xba, 0x31, 0xed, 0xd5, 0xa9, 0xbb, 0x96,
         0xd5, 0x0b, 0xcd, 0x7a, 0x3f, 0x96, 0x2a, 0x0f]);
    /// y = 2592331292931086675770238855846338635550719849568364935475441891787804997264
    pub static Y: Scalar = Scalar(
        [0x90, 0x76, 0x33, 0xfe, 0x1c, 0x4b, 0x66, 0xa4,
         0xa2, 0x8d, 0x2d, 0xd7, 0x67, 0x83, 0x86, 0xc3,
         0x53, 0xd0, 0xde, 0x54, 0x55, 0xd4, 0xfc, 0x9d,
         0xe8, 0xef, 0x7a, 0xc3, 0x1f, 0x35, 0xbb, 0x05]);
    /// z = 5033871415930814945849241457262266927579821285980625165479289807629491019013
    pub static Z: Scalar = Scalar(
        [0x05, 0x9d, 0x3e, 0x0b, 0x09, 0x26, 0x50, 0x3d,
         0xa3, 0x84, 0xa1, 0x3c, 0x92, 0x7a, 0xc2, 0x06,
         0x41, 0x98, 0xcf, 0x34, 0x3a, 0x24, 0xd5, 0xb7,
         0xeb, 0x33, 0x6a, 0x2d, 0xfc, 0x11, 0x21, 0x0b]);
    /// w = 3486911242272497535104403593250518247409663771668155364040899665266216860804
    static W: Scalar = Scalar(
        [0x84, 0xfc, 0xbc, 0x4f, 0x78, 0x12, 0xa0, 0x06,
         0xd7, 0x91, 0xd9, 0x7a, 0x3a, 0x27, 0xdd, 0x1e,
         0x21, 0x43, 0x45, 0xf7, 0xb1, 0xb9, 0x56, 0x7a,
         0x81, 0x30, 0x73, 0x44, 0x96, 0x85, 0xb5, 0x07]);

    /// x*y = 5690045403673944803228348699031245560686958845067437804563560795922180092780
    static X_TIMES_Y: Scalar = Scalar(
        [0x6c, 0x33, 0x74, 0xa1, 0x89, 0x4f, 0x62, 0x21,
         0x0a, 0xaa, 0x2f, 0xe1, 0x86, 0xa6, 0xf9, 0x2c,
         0xe0, 0xaa, 0x75, 0xc2, 0x77, 0x95, 0x81, 0xc2,
         0x95, 0xfc, 0x08, 0x17, 0x9a, 0x73, 0x94, 0x0c]);

    static A_SCALAR: Scalar = Scalar([
        0x1a, 0x0e, 0x97, 0x8a, 0x90, 0xf6, 0x62, 0x2d,
        0x37, 0x47, 0x02, 0x3f, 0x8a, 0xd8, 0x26, 0x4d,
        0xa7, 0x58, 0xaa, 0x1b, 0x88, 0xe0, 0x40, 0xd1,
        0x58, 0x9e, 0x7b, 0x7f, 0x23, 0x76, 0xef, 0x09]);

    static A_NAF: [i8; 256] =
        [0,13,0,0,0,0,0,0,0,7,0,0,0,0,0,0,-9,0,0,0,0,-11,0,0,0,0,3,0,0,0,0,1,
         0,0,0,0,9,0,0,0,0,-5,0,0,0,0,0,0,3,0,0,0,0,11,0,0,0,0,11,0,0,0,0,0,
         -9,0,0,0,0,0,-3,0,0,0,0,9,0,0,0,0,0,1,0,0,0,0,0,0,-1,0,0,0,0,0,9,0,
         0,0,0,-15,0,0,0,0,-7,0,0,0,0,-9,0,0,0,0,0,5,0,0,0,0,13,0,0,0,0,0,-3,0,
         0,0,0,-11,0,0,0,0,-7,0,0,0,0,-13,0,0,0,0,11,0,0,0,0,-9,0,0,0,0,0,1,0,0,
         0,0,0,-15,0,0,0,0,1,0,0,0,0,7,0,0,0,0,0,0,0,0,5,0,0,0,0,0,13,0,0,0,
         0,0,0,11,0,0,0,0,0,15,0,0,0,0,0,-9,0,0,0,0,0,0,0,-1,0,0,0,0,0,0,0,7,
         0,0,0,0,0,-15,0,0,0,0,0,15,0,0,0,0,15,0,0,0,0,15,0,0,0,0,0,1,0,0,0,0];

    #[test]
    fn fuzzer_testcase_reduction() {
        // LE bytes of 24519928653854221733733552434404946937899825954937634815
        let a_bytes = [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        // LE bytes of 4975441334397345751130612518500927154628011511324180036903450236863266160640
        let b_bytes = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 210, 210, 210, 255, 255, 255, 255, 10];
        // LE bytes of 6432735165214683820902750800207468552549813371247423777071615116673864412038
        let c_bytes = [134, 171, 119, 216, 180, 128, 178, 62, 171, 132, 32, 62, 34, 119, 104, 193, 47, 215, 181, 250, 14, 207, 172, 93, 75, 207, 211, 103, 144, 204, 56, 14];

        let a = Scalar(a_bytes);
        let b = Scalar(b_bytes);
        let c = Scalar(c_bytes);

        let mut tmp = [0u8; 64];

        // also_a = (a mod l)
        tmp[0..32].copy_from_slice(&a_bytes[..]);
        let also_a = Scalar::reduce(&tmp);

        // also_b = (b mod l)
        tmp[0..32].copy_from_slice(&b_bytes[..]);
        let also_b = Scalar::reduce(&tmp);

        let expected_c = &a * &b;
        let also_expected_c = &also_a * &also_b;

        assert_eq!(c, expected_c);
        assert_eq!(c, also_expected_c);
    }

    #[test]
    fn non_adjacent_form() {
        let naf = A_SCALAR.non_adjacent_form();
        for i in 0..256 {
            assert_eq!(naf[i], A_NAF[i]);
        }
    }

    #[test]
    fn from_unsigned() {
        let val = 0xdeadbeefdeadbeef;
        let s = Scalar::from_u64(val);
        assert_eq!(s[7], 0xde);
        assert_eq!(s[6], 0xad);
        assert_eq!(s[5], 0xbe);
        assert_eq!(s[4], 0xef);
        assert_eq!(s[3], 0xde);
        assert_eq!(s[2], 0xad);
        assert_eq!(s[1], 0xbe);
        assert_eq!(s[0], 0xef);
    }

    #[test]
    fn scalar_multiply_by_one() {
        let one = Scalar::one();
        let zero = Scalar::zero();
        let test_scalar = Scalar::multiply_add(&X, &one, &zero);
        for i in 0..32 {
            assert!(test_scalar[i] == X[i]);
        }
    }

    #[test]
    fn impl_add() {
        let mut two = Scalar::zero(); two[0] = 2;
        let two = two;
        let one = Scalar::one();
        let should_be_two = &one + &one;
        assert_eq!(should_be_two, two);
    }

    #[test]
    fn impl_sub() {
        let should_be_one = &constants::BASEPOINT_ORDER - &constants::BASEPOINT_ORDER_MINUS_1;
        assert_eq!(should_be_one, Scalar::one());
    }

    #[allow(non_snake_case)]
    #[test]
    fn impl_mul() {
        let should_be_X_times_Y = &X * &Y;
        assert_eq!(should_be_X_times_Y, X_TIMES_Y);
    }

    #[test]
    fn scalar_multiply_add() {
        let test_scalar = Scalar::multiply_add(&X, &Y, &Z);
        for i in 0..32 {
            assert!(test_scalar[i] == W[i]);
        }
    }

    #[test]
    fn square() {
        let expected = Scalar::multiply_add(&X, &X, &Scalar::zero());
        let actual = X.unpack().square().pack();
        for i in 0..32 {
            assert!(expected[i] == actual[i]);
        }
    }

    #[test]
    fn scalar_reduce() {
        let mut bignum = [0u8; 64];
        // set bignum = x + 2^256x
        for i in 0..32 {
            bignum[   i] = X[i];
            bignum[32+i] = X[i];
        }
        // 3958878930004874126169954872055634648693766179881526445624823978500314864344
        // = x + 2^256x (mod l)
        let reduced = Scalar([216, 154, 179, 139, 210, 121,   2,  71,
                               69,  99, 158, 216,  23, 173,  63, 100,
                              204,   0,  91,  50, 219, 153,  57, 249,
                               28,  82,  31, 197, 100, 165, 192,   8]);
        let test_red = Scalar::reduce(&bignum);
        for i in 0..32 {
            assert!(test_red[i] == reduced[i]);
        }
    }

    #[allow(non_snake_case)]
    #[test]
    fn invert() {
        let inv_X = X.invert();
        assert_eq!(inv_X, XINV);
        let should_be_one = &inv_X * &X;
        assert_eq!(should_be_one, Scalar::one());
    }

    // Negating a scalar twice should result in the original scalar.
    #[allow(non_snake_case)]
    #[test]
    fn neg_twice_is_identity() {
        let negative_X = -&X;
        let should_be_X = -&negative_X;

        assert_eq!(should_be_X, X);
    }

    #[test]
    fn to_bytes_from_bytes_roundtrips() {
        let unpacked = X.unpack();
        let bytes = unpacked.to_bytes();
        let should_be_unpacked = UnpackedScalar::from_bytes(&bytes);

        assert_eq!(should_be_unpacked.0, unpacked.0);
    }


    #[test]
    fn montgomery_reduce_matches_reduce() {
        let mut bignum = [0u8; 64];

        // set bignum = x + 2^256x
        for i in 0..32 {
            bignum[   i] = X[i];
            bignum[32+i] = X[i];
        }
        // x + 2^256x (mod l)
        //         = 3958878930004874126169954872055634648693766179881526445624823978500314864344
        let expected = Scalar([216, 154, 179, 139, 210, 121,   2,  71,
                                69,  99, 158, 216,  23, 173,  63, 100,
                               204,   0,  91,  50, 219, 153,  57, 249,
                                28,  82,  31, 197, 100, 165, 192,   8]);
        let reduced = Scalar::reduce(&bignum);

        // The reduced scalar should match the expected
        assert_eq!(reduced.0, expected.0);

        //  (x + 2^256x) * R
        let interim = UnpackedScalar::mul_internal(&UnpackedScalar::from_bytes_wide(&bignum),
                                                   &constants::R);
        // ((x + 2^256x) * R) / R  (mod l)
        let montgomery_reduced = UnpackedScalar::montgomery_reduce(&interim);

        // The Montgomery reduced scalar should match the reduced one, as well as the expected
        assert_eq!(montgomery_reduced.0, reduced.unpack().0);
        assert_eq!(montgomery_reduced.0, expected.unpack().0)
    }

    #[cfg(feature = "serde")]
    use serde_cbor;

    #[test]
    #[cfg(feature = "serde")]
    fn serde_cbor_scalar_roundtrip() {
        let output = serde_cbor::to_vec(&X).unwrap();
        let parsed: Scalar = serde_cbor::from_slice(&output).unwrap();
        assert_eq!(parsed, X);
    }
}

#[cfg(all(test, feature = "bench"))]
mod bench {
    use rand::OsRng;
    use test::Bencher;

    use super::*;
    use super::test::{X};

    #[bench]
    fn scalar_random(b: &mut Bencher) {
        let mut csprng: OsRng = OsRng::new().unwrap();

        b.iter(|| Scalar::random(&mut csprng));
    }

    #[bench]
    fn invert(b: &mut Bencher) {
        let x = X.unpack();
        b.iter(|| x.invert());
    }
}
