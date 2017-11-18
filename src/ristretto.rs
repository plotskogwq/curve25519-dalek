// -*- mode: rust; -*-
//
// This file is part of curve25519-dalek.
// Copyright (c) 2016-2017 Isis Lovecruft, Henry de Valence
// See LICENSE for licensing information.
//
// Authors:
// - Isis Agora Lovecruft <isis@patternsinthevoid.net>
// - Henry de Valence <hdevalence@hdevalence.ca>

//! An implementation of Ristretto, which provides a prime-order group.
//!
//! Ristretto is a modification of Mike Hamburg's [Decaf
//! cofactor-eliminating point-compression
//! scheme](https://eprint.iacr.org/2015/673.pdf) to work on top of the
//! Curve25519 group.
//!
//! Below are some notes on Ristretto, which are *NOT* a full writeup and which may have errors.
//!
//! # Notes on Ristretto
//!
//! ## Decaf
//!
//! The introduction of the Decaf paper, [_Decaf: Eliminating cofactors
//! through point compression_](https://eprint.iacr.org/2015/673.pdf)
//! notes that while most cryptographic systems require a group of prime
//! order, most concrete implementations using elliptic curve groups
//! fall short -- they either provide a group of prime order, but with
//! incomplete or variable-time addition formulae (for instance, most
//! Weierstrass models), or else they provide a fast and safe
//! implementation of a group whose order is not quite a prime \\(q\\),
//! but \\(hq\\) for a small cofactor \\(h\\) (for instance, Edwards
//! curves, which have cofactor at least \\(4\\)).
//!
//! This abstraction mismatch requires ad-hoc protocol modifications to
//! ensure security; these modifications require careful analysis and
//! are a recurring source of vulnerabilities.
//!
//! The Decaf suggestion is to use a quotient group, such as \\(\mathcal
//! E / \mathcal E[4]\\) or \\(2 \mathcal E / \mathcal E[2] \\), to
//! implement a prime-order group.
//!
//! This requires only changing
//!
//! 1. the function for equality checking (so that two representatives
//!    of the same coset are considered equal);
//! 2. the function for encoding (so that two representatives of the
//!    same coset are encoded as identical bitstrings);
//! 3. the function for decoding (so that only the canonical encoding of
//!    a coset is accepted).
//!
//! Internally, each coset is represented by a curve point; two points
//! may represent the same coset in the same way that two points with
//! different \\(X,Y,Z\\) coordinates may represent the same point.  The
//! group operations are carried out using the fast, safe Edwards
//! formulas.
//!
//! The Decaf paper suggests implementing the compression and
//! decompression routines using an isogeny from a Jacobi quartic; for
//! curves of cofactor \\(4\\), this eliminates the cofactor, and
//! explains the name: Decaf is named "after the procedure which divides
//! the effect of coffee by \\(4\\)".  However, Curve25519 has a
//! cofactor of \\(8\\).  To eliminate its cofactor, we tweak Decaf to
//! restrict further.  This gives the
//! [Ristretto](https://en.wikipedia.org/wiki/Ristretto) encoding.
//!
//! ## The Jacobi Quartic
//!
//! The Jacobi quartic is parameterized by \\(e, A\\), and is of the
//! form $$ \mathcal J\_{e,A} : t\^2 = es\^4 + 2As\^2 + 1, $$ with
//! identity point \\((0,1)\\).  For more details on the Jacobi quartic,
//! see the [Decaf paper](https://eprint.iacr.org/2015/673.pdf) or
//! [_Jacobi Quartic Curves
//! Revisited_](https://eprint.iacr.org/2009/312.pdf) by Hisil, Wong,
//! Carter, and Dawson).
//!
//! When \\(e = a\^2\\), \\(\mathcal J\_{e,A}\\) has full
//! \\(2\\)-torsion (i.e., \\(\mathcal J[2] \cong \mathbb Z /2 \times
//! \mathbb Z/2\\)), and
//! we can write the \\(\mathcal J[2]\\)-coset of a point \\(P =
//! (s,t)\\) as
//! $$
//! P + \mathcal J[2] = \left\\{
//!                       (s,t),
//!                       (-s,-t),
//!                       (1/as, -t/as\^2),
//!                       (-1/as, t/as\^2) \right\\}.
//! $$
//! Notice that replacing \\(a\\) by \\(-a\\) just swaps the last two
//! points, so this set does not depend on the choice of \\(a\\).  In
//! what follows we require \\(a = \pm 1\\).
//!
//! ## Encoding \\(\mathcal J / \mathcal J[2]\\)
//!
//! To encode points on \\(\mathcal J\\) modulo \\(\mathcal J[2]\\),
//! we need to choose a canonical representative of the above coset.
//! To do this, it's sufficient to make two independent sign choices:
//! the Decaf paper suggests choosing \\((s,t)\\) with \\(s\\)
//! non-negative and finite, and \\(t/s\\) non-negative or infinite.
//!
//! The encoding is then the (canonical byte encoding of the)
//! \\(s\\)-value of the canonical representative.
//!
//! ## The Edwards Curve
//!
//! Our primary internal model for Curve25519 points are the [_Extended
//! Twisted Edwards Coordinates_](https://eprint.iacr.org/2008/522.pdf)
//! of Hisil, Wong, Carter, and Dawson.
//! These correspond to the affine model
//!
//! $$\mathcal E\_{a,d} : ax\^2 + y\^2 = 1 + dx\^2y\^2.$$
//! 
//! In projective coordinates, we represent a point as \\((X:Y:Z:T)\\)
//! with $$XY = ZT, \quad aX\^2 + Y\^2 = Z\^2 + dT\^2.$$ (For more
//! details on this model, see the documentation for the `edwards`
//! module). The case \\(a = 1\\) is the _untwisted_ case; we only
//! consider \\(a = \pm 1\\), and in particular we focus on the twisted
//! Edwards form of Curve25519, which has \\(a = -1, d =
//! -121665/121666\\).  When not otherwise specified, we write
//! \\(\mathcal E\\) for \\(\mathcal E\_{-1, -121665/121666}\\).
//!
//! When both \\(d\\) and \\(ad\\) are nonsquare (which forces \\(a\\)
//! to be square), the curve is *complete*.  In this case the
//! four-torsion subgroup is cyclic, and we
//! can write it explicitly as
//! $$
//! \mathcal E\_{a,d}[4] = \\{ (0,1),\; (1/\sqrt a, 0),\; (0, -1),\; (-1/\sqrt{a}, 0)\\}.
//! $$
//! These are the only points with \\(xy = 0\\); the points with \\( y
//! \neq 0 \\) are \\(2\\)-torsion.  The \\(\mathcal
//! E\_{a,d}[4]\\)-coset of \\(P = (x,y)\\) is then
//! $$
//! P + \mathcal E\_{a,d}[4] = \\{ (x,y),\; (y/\sqrt a, -x\sqrt a),\; (-x, -y),\; (-y/\sqrt a, x\sqrt a)\\}.
//! $$
//! Notice that if \\(xy \neq 0 \\), then exactly two of
//! these points have \\( xy \\) non-negative, and they differ by the
//! \\(2\\)-torsion point \\( (0,-1) \\).  This means that we can select
//! a representative modulo \\(\mathcal
//! E\_{a,d}[2] \\) by requiring \\(xy\\) nonnegative and \\(y \neq
//! 0\\), and we can ensure this condition by conditionally adding a
//! \\(4\\)-torsion point if \\(xy\\) is negative or \\(y = 0\\).
//!
//! This procedure gives a canonical lift from \\(\mathcal E / \mathcal
//! E[4]\\) to \\(\mathcal E / \mathcal E[2]\\).  Since it involves a
//! conditional rotation, we refer to it as *torquing* the point.
//!
//! The structure of the Curve25519 group is \\( \mathcal E(\mathbb
//! F\_p) \cong \mathbb Z / 8 \times \mathbb Z / \ell\\), where \\( \ell
//! = 2\^{252} + \cdots \\) is a large prime.  Because \\(\mathcal E[8]
//! \cong \mathbb Z / 8\\), we have \\(\[2\](\mathcal E[8]) = \mathcal
//! E[4]\\), \\(\mathcal E[4] \cong \mathbb Z / 4
//! \\) and \\( \mathcal E[2] \cong \mathbb Z / 2\\).  In particular
//! this tells us that the group 
//! $$
//! \frac{\[2\](\mathcal E)}{\mathcal E[4]}
//! $$
//! is well-defined and has prime order \\( (8\ell / 2) / 4 = \ell \\).
//! This is the group we will construct using Ristretto.
//!
//! ## The Isogeny
//!
//! For \\(a = \pm 1\\), we have a \\(2\\)-isogeny
//! $$
//! \theta\_{a,d} : \mathcal J\_{a\^2, -a(a+d)/(a-d)} \longrightarrow \mathcal E\_{a,d}
//! $$
//! (or simply \\(\theta\\)) defined by
//! $$
//! \theta\_{a,d} : (s,t) \mapsto \left( \frac{1}{\sqrt{ad-1}} \cdot \frac{2s}{t},\quad \frac{1+as\^2}{1-as\^2} \right).
//! $$
//!
//! XXX Its dual is ... ?
//!
//! The kernel of the isogeny is \\( \{(0, \pm 1)\} \\).
//! The image of the isogeny is \\(\[2\](\mathcal E)\\).  To see this,
//! first note that because \\( \theta \circ \hat{\theta} = [2] \\), we
//! know that \\( \[2\](\mathcal E) \subseteq \theta(\mathcal J)\\); then, to see that
//! \\(\theta(\mathcal J)\\) is exactly \\(\[2\](\mathcal E)\\),
//! recall that isogenous elliptic curves over a finite field have the
//! same number of points (exercise 5.4 of Silverman), so that
//! $$
//! \\# \theta(\mathcal J) = \frac {\\# \mathcal J} {\\# \ker \theta}
//! = \frac {\\# \mathcal E}{2} = \\# \[2\](\mathcal E).
//! $$
//!
//! To determine the image \\(\theta(\mathcal J[2])\\) of the
//! \\(2\\)-torsion, we consider the image of the coset \\(\theta((s,t)
//! + \mathcal J[2])\\).  Let \\((x,y) = \theta(s,t)\\); then
//! \\(\theta(-s,-t) = (x,y)\\) and \\(\theta(1/as, -t/as\^2) = (-x,
//! -y)\\), so that \\(\theta(\mathcal J[2]) = \mathcal E[2]\\).
//!
//! The Decaf paper recalls that, for a group \\( G \\) with normal
//! subgroup \\(G' \leq G\\), a group homomorphism \\( \phi : G
//! \rightarrow H \\) induces a homomorphism
//! $$ 
//! \bar{\phi} : \frac G {G'} \longrightarrow \frac {\phi(G)}{\phi(G')} \leq \frac {H} {\phi(G')},
//! $$ 
//! and that the induced homomorphism \\(\bar{\phi}\\) is injective if
//! \\( \ker \phi \leq G' \\).  In our context, the kernel of
//! \\(\theta\\) is \\( \\{(0, \pm 1)\\} \leq \mathcal J[2] \\),
//! so \\(\theta\\) gives an isomorphism
//! $$
//! \frac {\mathcal J} {\mathcal J[2]} 
//! \cong 
//! \frac {\theta(\mathcal J)} {\theta(\mathcal J[2])}
//! \cong
//! \frac {\[2\](\mathcal E)} {\mathcal E[2]}.
//! $$
//!
//! We can use the isomorphism to transfer the encoding of \\(\mathcal
//! J / \mathcal J[2] \\) defined above to \\(\[2\](\mathcal E)/\mathcal
//! E[2]\\), by encoding the Edwards point \\((x,y)\\) using the Jacobi
//! quartic encoding of \\(\theta\^{-1}(x,y)\\).
//!
//! Since \\(\\# (\[2\](\mathcal E) / \mathcal E[2]) = (\\#\mathcal
//! E)/4\\), if \\(\mathcal E\\) has cofactor \\(4\\), we're done.
//! Otherwise, if \\(\mathcal E\\) has cofactor \\(8\\), as in the
//! Curve25519 case, we use the torquing procedure to lift \\(\mathcal E
//! / \mathcal E[4]\\) to \\(\mathcal E / \mathcal E[2]\\), and then
//! apply the encoding for \\( \[2\](\mathcal E) / \mathcal E[2] \\).
//!
//! ## The Ristretto Encoding
//!
//! We can write the above encoding/decoding procedure concretely (in affine
//! coordinates) as follows:
//!
//! ### Encoding
//!
//! On input \\( (x,y) \in \[2\](\mathcal E)\\), a representative for a
//! coset in \\( \[2\](\mathcal E) / \mathcal E[4] \\):
//!
//! 1. Check if \\( xy \\) is negative or \\( x = 0 \\); if so, torque
//!    the point by setting \\( (x,y) \gets (x,y) + P_4 \\), where
//!    \\(P_4\\) is a \\(4\\)-torsion point.
//!
//! 2. Check if \\(x\\) is negative or \\( y = -1 \\); if so, set 
//!    \\( (x,y) \gets (x,y) + (0,-1) = (-x, -y) \\).
//!
//! 3. Compute $$ s = +\sqrt {(-a) \frac {1 - y} {1 + y} }, $$ choosing
//!    the positive square root.
//!
//! The output is then the (canonical) byte-encoding of \\(s\\).  
//!
//! If \\(\mathcal E\\) has cofactor \\(4\\), we skip the first step,
//! since our input already represents a coset in
//! \\( \[2\](\mathcal E) / \mathcal E[2] \\).
//! 
//! To see that this corresponds to the encoding procedure above, notice
//! that the first step lifts from \\( \mathcal E / \mathcal E[4] \\) to
//! \\(\mathcal E / \mathcal E[2]\\).  To understand steps 2 and 3,
//! notice that the \\(y\\)-coordinate of \\(\theta(s,t)\\) is 
//! $$
//! y = \frac {1 + as\^2}{1 - as\^2},
//! $$
//! so that the \\(s\\)-coordinate of \\(\theta\^{-1}(x,y)\\) has 
//! $$
//! s\^2 = (-a)\frac {1-y}{1+y}.
//! $$
//! Since
//! $$
//! x = \frac 1 {\sqrt {ad - 1}} \frac {2s} {t},
//! $$
//! we also have
//! $$
//! \frac s t = x \frac {\sqrt {ad-1}} 2,
//! $$
//! so that the sign of \\(s/t\\) is determined by the sign of \\(x\\).
//!
//! Recall that to choose a canonical representative of \\( (s,t) +
//! \mathcal J[2] \\), it's sufficient to make two sign choices: the
//! sign of \\(s\\) and the sign of \\(s/t\\).  Step 2 determines the
//! sign of \\(s/t\\), while step 3 computes \\(s\\) and determines its
//! sign (by choosing the positive square root).  Finally, the check
//! that \\(y \neq -1\\) prevents division-by-zero when encoding the
//! identity; it falls out of the optimized formulas below.
//!
//! ### Decoding
//!
//! On input `s_bytes`, decoding proceeds as follows:
//!
//! 1. Decode `s_bytes` to \\(s\\); reject if `s_bytes` is not the
//!    canonical encoding of \\(s\\).
//!
//! 2. Check whether \\(s\\) is negative; if so, reject.
//!
//! 3. Compute
//! $$
//! y \gets \frac {1 + as\^2}{1 - as\^2}.
//! $$
//!
//! 4. Compute
//! $$
//! x \gets +\sqrt{ \frac{4s\^2} {ad(1+as\^2)\^2 - (1-as\^2)\^2}},
//! $$
//! choosing the positive square root, or reject if the square root does
//! not exist.
//!
//! 5. Check whether \\(xy\\) is negative or \\(y = 0\\); if so, reject.
//!
//! ## Encoding in Extended Coordinates
//!
//! The formulas above are given in affine coordinates, but the usual
//! internal representation is extended twisted Edwards coordinates \\(
//! (X:Y:Z:T) \\) with \\( x = X/Z \\), \\(y = Y/Z\\), \\(xy = T/Z \\).
//! Selecting the distinguished representative of the coset
//! requires the affine coordinates \\( (x,y) \\), and computing \\( s
//! \\) requires an inverse square root.
//! As inversions are expensive, we'd like to be able to do this
//! whole computation with only one inverse square root, by batching
//! together the inversion and the inverse square root.  
//!
//! However, it is not obvious how to do this, since the inverse square
//! root computation depends on the affine coordinates (which select the
//! distinguished representative).
//!
//! In what follows we consider only the case
//! \\(a = -1\\); a similar argument applies to the case \\( a = 1\\).
//!
//! Since \\(y = Y/Z\\), in extended coordinates the formula for \\(s\\) becomes
//! $$
//! s = \sqrt{ \frac{ 1 - Y/Z}{1+Y/Z}} = \sqrt{\frac{Z - Y}{Z+Y}}
//! = \frac {Z - Y} {\sqrt{Z\^2 - Y\^2}}.
//! $$
//!
//! Here \\( (X:Y:Z:T) \\) are the coordinates of the distinguished
//! representative of the coset.  
//! Write \\( (X\_0 : Y\_0 : Z\_0 : T\_0) \\)
//! for the coordinates of the initial representative.  Then the
//! torquing procedure in step 1 replaces \\( (X\_0 : Y\_0 : Z\_0 :
//! T\_0) \\) by \\( (iY\_0 : iX\_0 : Z\_0 : -T\_0) \\).  This means we
//! want to obtain either
//! $$
//! \frac {1} { \sqrt{Z\_0\^2 - Y\_0\^2}}
//! \quad \text{or} \quad
//! \frac {1} { \sqrt{Z\_0\^2 + X\_0\^2}}.
//! $$
//!
//! We can relate these using the identity
//! $$
//! (a-d)X\^2Y\^2 = (Z\^2 - aX\^2)(Z\^2 - Y\^2),
//! $$
//! which is valid for all curve points.  To see this, recall from the curve equation that
//! $$
//! -dX\^2Y\^2 = Z\^4 - aZ\^2X\^2 - Z\^2Y\^2,
//! $$
//! so that
//! $$
//! (a-d)X\^2Y\^2 = Z\^4 - aZ\^2X\^2 - Z\^2Y\^2 + aX\^2Y\^2 = (Z\^2 - Y\^2)(Z\^2 + X\^2).
//! $$
//! 
//! The encoding procedure is as follows:
//! 
//! 1. \\(u\_1 \gets (Z\_0 + Y\_0)(Z\_0 - Y\_0) = Z\_0\^2 - Y\_0\^2 \\)
//! 2. \\(u\_2 \gets X\_0 Y\_0 \\)
//! 3. \\(I \gets \mathrm{invsqrt}(u\_1 u\_2\^2) = 1/\sqrt{X\_0\^2 Y\_0\^2 (Z\_0\^2 - Y\_0\^2)} \\)
//! 4. \\(D\_1 \gets u\_1 I = \sqrt{(Z\_0\^2 - Y\_0\^2)/(X\_0\^2 Y\_0\^2)} \\)
//! 5. \\(D\_2 \gets u\_2 I = \pm \sqrt{1/(Z\_0\^2 - Y\_0\^2)} \\)
//! 6. \\(Z\_{inv} \gets D\_1 D\_2 T\_0 = (u\_1 u\_2)/(u\_1 u\_2\^2) T\_0 = T\_0 / X\_0 Y\_0 = 1/Z\_0 \\)
//! 7. If \\( T\_0 Z\_{inv} = x\_0 y\_0 \\) is negative:
//!     1. \\( X \gets iY\_0 \\)
//!     2. \\( Y \gets iX\_0 \\)
//!     3. \\( D \gets D\_1 / \sqrt{a-d} = 1/\sqrt{Z\_0\^2 + X\_0\^2} \\)
//! 8. Otherwise:
//!     1. \\( X \gets X\_0 \\)
//!     2. \\( Y \gets Y\_0 \\)
//!     3. \\( D \gets D\_2 = \pm \sqrt{1/(Z\_0\^2 - Y\_0\^2)} \\)
//! 9. If \\( X Z\_{inv} = x \\) is negative, set \\( Y \gets - Y\\)
//! 10. Compute \\( s \gets (Z - Y) D = (Z - Y) / \sqrt{Z\^2 - Y\^2} \\) and return.
//!
//! ## Decoding to Extended Coordinates
//!
//! ## Equality Testing
//!
//! ## Elligator
//!
//! ## The Double-Ristretto Encoding
//!
//! It's possible to do batch encoding of \\( [2]P \\) using the dual
//! isogeny \\(\hat{\theta}\\).  Defer this for now.
//!
//! ## ???

// We allow non snake_case names because coordinates in projective space are
// traditionally denoted by the capitalisation of their respective
// counterparts in affine space.  Yeah, you heard me, rustc, I'm gonna have my
// affine and projective cakes and eat both of them too.
#![allow(non_snake_case)]

use core::fmt::Debug;

#[cfg(feature = "std")]
use rand::Rng;

use digest::Digest;
use generic_array::typenum::U32;

use constants;
use field::FieldElement;

use core::ops::{Add, Sub, Neg};
use core::ops::{AddAssign, SubAssign};
use core::ops::{Mul, MulAssign};

use edwards;
use edwards::ExtendedPoint;
use edwards::CompletedPoint;
use edwards::EdwardsBasepointTable;
use edwards::Identity;
use scalar::Scalar;

use subtle;
use subtle::ConditionallyAssignable;
use subtle::ConditionallyNegatable;
use subtle::Equal;

// ------------------------------------------------------------------------
// Compressed points
// ------------------------------------------------------------------------

/// A point serialized using Mike Hamburg's Ristretto scheme.
///
/// XXX think about how this API should work
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct CompressedRistretto(pub [u8; 32]);

/// The result of compressing a `RistrettoPoint`.
impl CompressedRistretto {
    /// View this `CompressedRistretto` as an array of bytes.
    pub fn as_bytes<'a>(&'a self) -> &'a [u8; 32] {
        &self.0
    }

    /// Attempt to decompress to an `RistrettoPoint`.
    ///
    /// This function executes in constant time for all valid inputs.
    /// Inputs which do not decode to a RistrettoPoint may return
    /// early.
    pub fn decompress(&self) -> Option<RistrettoPoint> {
        // Step 1. Check s for validity:
        // 1.a) s must be 32 bytes (we get this from the type system)
        // 1.b) s < p
        // 1.c) s is nonnegative
        //
        // Our decoding routine ignores the high bit, so the only
        // possible failure for 1.b) is if someone encodes s in 0..18
        // as s+p in 2^255-19..2^255-1.  We can check this by
        // converting back to bytes, and checking that we get the
        // original input, since our encoding routine is canonical.

        let s = FieldElement::from_bytes(self.as_bytes());
        let s_bytes_check = s.to_bytes();
        let s_encoding_is_canonical =
            subtle::slices_equal(&s_bytes_check[..], self.as_bytes());
        let s_is_negative = s.is_negative();

        if s_encoding_is_canonical == 0u8 || s_is_negative == 1u8 {
            return None;
        }

        // Step 2.  The rest.  (XXX write comments)
        let one = FieldElement::one();
        let ss = s.square();
        let yden = &one + &ss; // 1 - a*s^2
        let ynum = &one - &ss; // 1 + a*s^2
        let yden_sqr = yden.square();
        let xden_sqr = &(&(-&constants::EDWARDS_D) * &ynum.square()) - &yden_sqr;

        let (ok, invsqrt) = (&xden_sqr * &yden_sqr).invsqrt();

        let xden_inv = &invsqrt * &yden;
        let yden_inv = &invsqrt * &(&xden_inv * &xden_sqr);

        let mut x = &(&s + &s) * &xden_inv; // 2*s*xden_inv
        let x_is_negative = x.is_negative();
        x.conditional_negate(x_is_negative);
        let y = &ynum * &yden_inv;

        let t = &x * &y;

        if ok == 0u8 || t.is_negative() == 1u8 || y.is_zero() == 1u8 {
            return None;
        } else {
            return Some(RistrettoPoint(ExtendedPoint{X: x, Y: y, Z: one, T: t}));
        }
    }
}

impl Identity for CompressedRistretto {
    fn identity() -> CompressedRistretto {
        CompressedRistretto([0u8; 32])
    }
}

// ------------------------------------------------------------------------
// Serde support
// ------------------------------------------------------------------------
// Serializes to and from `RistrettoPoint` directly, doing compression
// and decompression internally.  This means that users can create
// structs containing `RistrettoPoint`s and use Serde's derived
// serializers to serialize those structures.

#[cfg(feature = "serde")]
use serde::{self, Serialize, Deserialize, Serializer, Deserializer};
#[cfg(feature = "serde")]
use serde::de::Visitor;

#[cfg(feature = "serde")]
impl Serialize for RistrettoPoint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_bytes(self.compress().as_bytes())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for RistrettoPoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        struct RistrettoPointVisitor;

        impl<'de> Visitor<'de> for RistrettoPointVisitor {
            type Value = RistrettoPoint;

            fn expecting(&self, formatter: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                formatter.write_str("a valid point in Ristretto format")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<RistrettoPoint, E>
                where E: serde::de::Error
            {
                if v.len() == 32 {
                    let arr32 = array_ref!(v, 0, 32); // &[u8;32] from &[u8]
                    CompressedRistretto(*arr32)
                        .decompress()
                        .ok_or(serde::de::Error::custom("decompression failed"))
                } else {
                    Err(serde::de::Error::invalid_length(v.len(), &self))
                }
            }
        }

        deserializer.deserialize_bytes(RistrettoPointVisitor)
    }
}

// ------------------------------------------------------------------------
// Internal point representations
// ------------------------------------------------------------------------

/// A `RistrettoPoint` represents a point in the Ristretto group for
/// Curve25519.  Ristretto, a variant of Decaf, constructs a
/// prime-order group as a quotient group of a subgroup of (the
/// Edwards form of) Curve25519.
///
/// Internally, a `RistrettoPoint` is a wrapper type around
/// `ExtendedPoint`, with custom equality, compression, and
/// decompression routines to account for the quotient.
#[derive(Copy, Clone)]
pub struct RistrettoPoint(pub ExtendedPoint);

impl RistrettoPoint {
    /// Compress in Ristretto format.
    ///
    /// # Implementation Notes
    ///
    /// The Ristretto encoding is as follows, on input in affine coordinates `(x,y)`:
    ///
    /// 1.  If `xy` is negative or `x = 0`, "rotate" the point by
    /// setting `(x,y) = (iy, ix)`.
    /// 2.  If `x` is negative, set `(x,y) = (-x, -y)`.
    /// 3.  Compute `s = +sqrt((1-y)/(1+y))`.
    /// 4.  Return the little-endian 32-byte encoding of `s`.
    ///
    /// However, our input is in extended twisted Edwards coordinates
    /// `(X:Y:Z:T)` with `x = X/Z`, `y = Y/Z`, `xy = T/Z` (see the
    /// module-level documentation on curve representations for more
    /// details).  Since inversions are expensive, we'd like to be
    /// able to do this whole computation with only one inversion.
    ///
    /// Since `y = Y/Z`, in extended coordinates the formula for `s` becomes
    ///
    ///     s = sqrt((1 - Y/Z)/(1 + Y/Z)) = sqrt((Z-Y)/(Z+Y)).  <span style="float: right">(1)</span>
    ///
    /// We can compute this as
    ///
    ///     s = (Z - Y) / sqrt((Z-Y)(Z+Y)).  <span style="float: right">(1)</span>
    ///
    /// The denominator is 
    ///
    ///      invsqrt((Z-Y)(Z+Y)) = invsqrt(Z² - Y²).  <span style="float: right">(1)</span>
    ///
    /// Write the input point as `(X₀:Y₀:Z₀:T₀)`.  The rotation in
    /// step 1 of the encoding procedure replaces `(X₀:Y₀:Z₀:T₀)` by
    /// `(iY₀:iX₀:Z₀:-T₀)`.  We therefore wish to relate the
    /// computation of
    ///
    ///      invsqrt(Z² - Y²) = invsqrt(Z₀² - Y₀²)  [non-rotated case]
    ///
    /// with the computation of
    ///
    ///      invsqrt(Z² - Y²) = invsqrt(Z₀² + X₀²).  [rotated case]
    ///
    /// Recall the curve equation (in the 𝗣² model):
    ///
    ///     (-X² + Y²)Z² = Z⁴ + dX²Y².  <span style="float: right">(1)</span>
    ///
    /// This means that, for any point `(X:Y:Z:T)` in extended coordinates, we have
    ///
    ///     -dX²Y² = Z⁴ + Z²X² - Z²Y²,  <span style="float: right">(2)</span>
    ///
    /// so that 
    ///
    ///     (-1-d)X²Y² = Z⁴ + Z²X² - Z²Y² - X²Y²,  <span style="float: right">(3)</span>
    ///
    /// and hence
    ///
    ///     (-1-d)X²Y² = (Z² - Y²)(Z² + X²).  <span style="float: right">(4)</span>
    ///
    /// Taking inverse square roots gives
    ///
    ///     invsqrt(Z² + X²) = invsqrt(-1-d) sqrt((Z² - Y²)/(X²Y²)). <span style="float: right">(4)</span>
    /// 
    ///
    pub fn compress(&self) -> CompressedRistretto {
        let mut X = self.0.X;
        let mut Y = self.0.Y;
        let Z = &self.0.Z;
        let T = &self.0.T;

        let u1 = &(Z + &Y) * &(Z - &Y);
        let u2 = &X * &Y;
        // Ignore return value since this is always square
        let (_, invsqrt) = (&u1 * &u2.square()).invsqrt();
        let i1 = &invsqrt * &u1;
        let i2 = &invsqrt * &u2;
        let z_inv = &i1 * &(&i2 * T);
        let mut den_inv = i2;

        let iX = &X * &constants::SQRT_M1;
        let iY = &Y * &constants::SQRT_M1;
        let ristretto_magic = &constants::INVSQRT_A_MINUS_D;
        let enchanted_denominator = &i1 * ristretto_magic;

        let rotate = (T * &z_inv).is_negative();

        X.conditional_assign(&iY, rotate);
        Y.conditional_assign(&iX, rotate);
        den_inv.conditional_assign(&enchanted_denominator, rotate);

        Y.conditional_negate((&X * &z_inv).is_negative());

        let mut s = &den_inv * &(Z - &Y);
        let s_is_negative = s.is_negative();
        s.conditional_negate(s_is_negative);

        CompressedRistretto(s.to_bytes())
    }

    /// Return the coset self + E[4], for debugging.
    fn coset4(&self) -> [ExtendedPoint; 4] {
        [  self.0
        , &self.0 + &constants::EIGHT_TORSION[2]
        , &self.0 + &constants::EIGHT_TORSION[4]
        , &self.0 + &constants::EIGHT_TORSION[6]
        ]
    }

    /// Computes the Ristretto Elligator map.
    ///
    /// # Note
    ///
    /// This method is not public because it's just used for hashing
    /// to a point -- proper elligator support is deferred for now.
    pub fn elligator_ristretto_flavour(r_0: &FieldElement) -> RistrettoPoint {
        let (i, d) = (&constants::SQRT_M1, &constants::EDWARDS_D);
        let one = FieldElement::one();

        let r = i * &r_0.square();

        // D = (dr -a)(ar-d) = -(dr+1)(r+d) 
        let D = -&( &(&(d * &r) + &one) * &(&r + d) );
        // N = a(d-a)(d+a)(r+1) = -(r+1)(d^2 -1)
        let d_sq = d.square();
        let N = -&( &(&d_sq - &one) * &(&r + &one) );

        let mut s = FieldElement::zero();
        let mut c = -&one;

        let (N_over_D_is_square, maybe_s) = FieldElement::sqrt_ratio(&N, &D);
        // s = sqrt(N/D) if N/D is square
        s.conditional_assign(&maybe_s, N_over_D_is_square);

        // XXX how do we reuse the computation of sqrt(N/D) to find sqrt(rN/D) ?
        let (rN_over_D_is_square, mut maybe_s) = FieldElement::sqrt_ratio(&(&r*&N), &D);
        maybe_s.negate();

        // s = -sqrt(rN/D) if rN/D is square (should happen exactly when N/D is nonsquare)
        debug_assert_eq!(N_over_D_is_square ^ rN_over_D_is_square, 1u8);
        s.conditional_assign(&maybe_s, rN_over_D_is_square);
        c.conditional_assign(&r, rN_over_D_is_square);

        // T = (c * (r - one) * (d-one).square()) - D;
        let T = &(&c * &(&(&r - &one) * &((d - &one).square()))) - &D;

        let s_sq = s.square();
        let P = CompletedPoint{
            X: &(&s + &s) * &D,
            Z: &T * &constants::SQRT_AD_MINUS_ONE,
            Y: &FieldElement::one() - &s_sq,
            T: &FieldElement::one() + &s_sq,
        };

        // Convert to extended and return.
        RistrettoPoint(P.to_extended())
    }

    /// Return a `RistrettoPoint` chosen uniformly at random using a user-provided RNG.
    ///
    /// # Inputs
    ///
    /// * `rng`: any RNG which implements the `rand::Rng` interface.
    ///
    /// # Returns
    ///
    /// A random element of the Ristretto group.
    ///
    /// # Implementation
    ///
    /// Uses the Ristretto-flavoured Elligator 2 map, so that the discrete log of the
    /// output point with respect to any other point should be unknown.
    #[cfg(feature = "std")]
    pub fn random<T: Rng>(rng: &mut T) -> Self {
        let mut field_bytes = [0u8; 32];
        rng.fill_bytes(&mut field_bytes);
        let r_0 = FieldElement::from_bytes(&field_bytes);
        RistrettoPoint::elligator_ristretto_flavour(&r_0)
    }

    /// Hash a slice of bytes into a `RistrettoPoint`.
    ///
    /// Takes a type parameter `D`, which is any `Digest` producing 32
    /// bytes (256 bits) of output.
    ///
    /// Convenience wrapper around `from_hash`.
    ///
    /// # Implementation
    ///
    /// Uses the Ristretto-flavoured Elligator 2 map, so that the discrete log of the
    /// output point with respect to any other point should be unknown.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate curve25519_dalek;
    /// # use curve25519_dalek::ristretto::RistrettoPoint;
    /// extern crate sha2;
    /// use sha2::Sha256;
    ///
    /// # // Need fn main() here in comment so the doctest compiles
    /// # // See https://doc.rust-lang.org/book/documentation.html#documentation-as-tests
    /// # fn main() {
    /// let msg = "To really appreciate architecture, you may even need to commit a murder";
    /// let P = RistrettoPoint::hash_from_bytes::<Sha256>(msg.as_bytes());
    /// # }
    /// ```
    ///
    pub fn hash_from_bytes<D>(input: &[u8]) -> RistrettoPoint
        where D: Digest<OutputSize = U32> + Default
    {
        let mut hash = D::default();
        hash.input(input);
        RistrettoPoint::from_hash(hash)
    }

    /// Construct a `RistrettoPoint` from an existing `Digest` instance.
    ///
    /// Use this instead of `hash_from_bytes` if it is more convenient
    /// to stream data into the `Digest` than to pass a single byte
    /// slice.
    pub fn from_hash<D>(hash: D) -> RistrettoPoint
        where D: Digest<OutputSize = U32> + Default
    {
        // XXX this seems clumsy
        let mut output = [0u8; 32];
        output.copy_from_slice(hash.result().as_slice());
        let r_0 = FieldElement::from_bytes(&output);
        RistrettoPoint::elligator_ristretto_flavour(&r_0)
    }
}

impl Identity for RistrettoPoint {
    fn identity() -> RistrettoPoint {
        RistrettoPoint(ExtendedPoint::identity())
    }
}

// ------------------------------------------------------------------------
// Equality
// ------------------------------------------------------------------------

impl PartialEq for RistrettoPoint {
    fn eq(&self, other: &RistrettoPoint) -> bool {
        self.ct_eq(other) == 1u8
    }
}

impl Equal for RistrettoPoint {
    /// Test equality between two `RistrettoPoint`s.
    ///
    /// # Returns
    ///
    /// `1u8` if the two `RistrettoPoint`s are equal, and `0u8` otherwise.
    fn ct_eq(&self, other: &RistrettoPoint) -> u8 {
        let X1Y2 = &self.0.X * &other.0.Y;
        let Y1X2 = &self.0.Y * &other.0.X;
        let X1X2 = &self.0.X * &other.0.X;
        let Y1Y2 = &self.0.Y * &other.0.Y;
        
        X1Y2.ct_eq(&Y1X2) | X1X2.ct_eq(&Y1Y2)
    }
}

impl Eq for RistrettoPoint {}

// ------------------------------------------------------------------------
// Arithmetic
// ------------------------------------------------------------------------

impl<'a, 'b> Add<&'b RistrettoPoint> for &'a RistrettoPoint {
    type Output = RistrettoPoint;

    fn add(self, other: &'b RistrettoPoint) -> RistrettoPoint {
        RistrettoPoint(&self.0 + &other.0)
    }
}

impl<'b> AddAssign<&'b RistrettoPoint> for RistrettoPoint {
    fn add_assign(&mut self, _rhs: &RistrettoPoint) {
        *self = (self as &RistrettoPoint) + _rhs;
    }
}

impl<'a, 'b> Sub<&'b RistrettoPoint> for &'a RistrettoPoint {
    type Output = RistrettoPoint;

    fn sub(self, other: &'b RistrettoPoint) -> RistrettoPoint {
        RistrettoPoint(&self.0 - &other.0)
    }
}

impl<'b> SubAssign<&'b RistrettoPoint> for RistrettoPoint {
    fn sub_assign(&mut self, _rhs: &RistrettoPoint) {
        *self = (self as &RistrettoPoint) - _rhs;
    }
}

impl<'a> Neg for &'a RistrettoPoint {
    type Output = RistrettoPoint;

    fn neg(self) -> RistrettoPoint {
        RistrettoPoint(-&self.0)
    }
}

impl<'b> MulAssign<&'b Scalar> for RistrettoPoint {
    fn mul_assign(&mut self, scalar: &'b Scalar) {
        let result = (self as &RistrettoPoint) * scalar;
        *self = result;
    }
}

impl<'a, 'b> Mul<&'b Scalar> for &'a RistrettoPoint {
    type Output = RistrettoPoint;
    /// Scalar multiplication: compute `scalar * self`.
    fn mul(self, scalar: &'b Scalar) -> RistrettoPoint {
        RistrettoPoint(&self.0 * scalar)
    }
}

impl<'a, 'b> Mul<&'b RistrettoPoint> for &'a Scalar {
    type Output = RistrettoPoint;

    /// Scalar multiplication: compute `self * scalar`.
    fn mul(self, point: &'b RistrettoPoint) -> RistrettoPoint {
        RistrettoPoint(self * &point.0)
    }
}

/// Given a vector of (possibly secret) scalars and a vector of
/// (possibly secret) points, compute `c_1 P_1 + ... + c_n P_n`.
///
/// This function has the same behaviour as
/// `vartime::multiscalar_mult` but is constant-time.
///
/// # Input
///
/// An iterable of `Scalar`s and a iterable of `DecafPoints`.  It is an
/// error to call this function with two iterators of different lengths.
#[cfg(any(feature = "alloc", feature = "std"))]
pub fn multiscalar_mult<'a, 'b, I, J>(scalars: I, points: J) -> RistrettoPoint
    where I: IntoIterator<Item = &'a Scalar>,
          J: IntoIterator<Item = &'b RistrettoPoint>,
{
    let extended_points = points.into_iter().map(|P| &P.0);
    RistrettoPoint(edwards::multiscalar_mult(scalars, extended_points))
}

/// Precomputation
#[derive(Clone)]
pub struct RistrettoBasepointTable(pub EdwardsBasepointTable);

impl<'a, 'b> Mul<&'b Scalar> for &'a RistrettoBasepointTable {
    type Output = RistrettoPoint;

    fn mul(self, scalar: &'b Scalar) -> RistrettoPoint {
        RistrettoPoint(&self.0 * scalar)
    }
}

impl<'a, 'b> Mul<&'a RistrettoBasepointTable> for &'b Scalar {
    type Output = RistrettoPoint;

    fn mul(self, basepoint_table: &'a RistrettoBasepointTable) -> RistrettoPoint {
        RistrettoPoint(self * &basepoint_table.0)
    }
}

impl RistrettoBasepointTable {
    /// Create a precomputed table of multiples of the given `basepoint`.
    pub fn create(basepoint: &RistrettoPoint) -> RistrettoBasepointTable {
        RistrettoBasepointTable(EdwardsBasepointTable::create(&basepoint.0))
    }

    /// Get the basepoint for this table as a `RistrettoPoint`.
    pub fn basepoint(&self) -> RistrettoPoint {
        RistrettoPoint(self.0.basepoint())
    }
}

// ------------------------------------------------------------------------
// Constant-time conditional assignment
// ------------------------------------------------------------------------

impl ConditionallyAssignable for RistrettoPoint {
    /// Conditionally assign `other` to `self`, if `choice == 1u8`.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate subtle;
    /// # extern crate curve25519_dalek;
    /// #
    /// # use subtle::ConditionallyAssignable;
    /// #
    /// # use curve25519_dalek::edwards::Identity;
    /// # use curve25519_dalek::ristretto::RistrettoPoint;
    /// # use curve25519_dalek::constants;
    /// # fn main() {
    /// let A = RistrettoPoint::identity();
    /// let B = constants::RISTRETTO_BASEPOINT_POINT;
    ///
    /// let mut P = A;
    ///
    /// P.conditional_assign(&B, 0u8);
    /// assert!(P == A);
    /// P.conditional_assign(&B, 1u8);
    /// assert!(P == B);
    /// # }
    /// ```
    fn conditional_assign(&mut self, other: &RistrettoPoint, choice: u8) {
        self.0.X.conditional_assign(&other.0.X, choice);
        self.0.Y.conditional_assign(&other.0.Y, choice);
        self.0.Z.conditional_assign(&other.0.Z, choice);
        self.0.T.conditional_assign(&other.0.T, choice);
    }
}

// ------------------------------------------------------------------------
// Debug traits
// ------------------------------------------------------------------------

impl Debug for CompressedRistretto {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        write!(f, "CompressedRistretto: {:?}", self.as_bytes())
    }
}

impl Debug for RistrettoPoint {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        let coset = self.coset4();
        write!(f, "RistrettoPoint: coset \n{:?}\n{:?}\n{:?}\n{:?}",
               coset[0], coset[1], coset[2], coset[3])
    }
}

// ------------------------------------------------------------------------
// Variable-time functions
// ------------------------------------------------------------------------

pub mod vartime {
    //! Variable-time operations on ristretto points, useful for non-secret data.
    use super::*;

    /// Given a vector of public scalars and a vector of (possibly secret)
    /// points, compute
    ///
    ///    c_1 P_1 + ... + c_n P_n.
    ///
    /// # Input
    ///
    /// A vector of `Scalar`s and a vector of `RistrettoPoints`.  It is an
    /// error to call this function with two vectors of different lengths.
    #[cfg(any(feature = "alloc", feature = "std"))]
    pub fn multiscalar_mult<'a, 'b, I, J>(scalars: I, points: J) -> RistrettoPoint
        where I: IntoIterator<Item = &'a Scalar>,
              J: IntoIterator<Item = &'b RistrettoPoint>
    {
        let extended_points = points.into_iter().map(|P| &P.0);
        RistrettoPoint(edwards::vartime::multiscalar_mult(scalars, extended_points))
    }
}

// ------------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use rand::OsRng;

    use scalar::Scalar;
    use constants;
    use edwards::CompressedEdwardsY;
    use edwards::Identity;
    use edwards::ValidityCheck;
    use super::*;

    #[cfg(feature = "serde")]
    use serde_cbor;

    #[test]
    #[cfg(feature = "serde")]
    fn serde_cbor_basepoint_roundtrip() {
        let output = serde_cbor::to_vec(&constants::RISTRETTO_BASEPOINT_POINT).unwrap();
        let parsed: RistrettoPoint = serde_cbor::from_slice(&output).unwrap();
        assert_eq!(parsed, constants::RISTRETTO_BASEPOINT_POINT);
    }

    #[test]
    fn scalarmult_ristrettopoint_works_both_ways() {
        let P = constants::RISTRETTO_BASEPOINT_POINT;
        let s = Scalar::from_u64(999);

        let P1 = &P * &s;
        let P2 = &s * &P;

        assert!(P1.compress().as_bytes() == P2.compress().as_bytes());
    }

    #[test]
    fn decompress_negative_s_fails() {
        // constants::d is neg, so decompression should fail as |d| != d.
        let bad_compressed = CompressedRistretto(constants::EDWARDS_D.to_bytes());
        assert!(bad_compressed.decompress().is_none());
    }

    #[test]
    fn decompress_id() {
        let compressed_id = CompressedRistretto::identity();
        let id = compressed_id.decompress().unwrap();
        let mut identity_in_coset = false;
        for P in &id.coset4() {
            if P.compress() == CompressedEdwardsY::identity() {
                identity_in_coset = true;
            }
        }
        assert!(identity_in_coset);
    }

    #[test]
    fn compress_id() {
        let id = RistrettoPoint::identity();
        assert_eq!(id.compress(), CompressedRistretto::identity());
    }

    #[test]
    fn basepoint_roundtrip() {
        let bp_compressed_ristretto = constants::RISTRETTO_BASEPOINT_POINT.compress();
        let bp_recaf = bp_compressed_ristretto.decompress().unwrap().0;
        // Check that bp_recaf differs from bp by a point of order 4
        let diff = &constants::RISTRETTO_BASEPOINT_POINT.0 - &bp_recaf;
        let diff4 = diff.mult_by_pow_2(2);
        assert_eq!(diff4.compress(), CompressedEdwardsY::identity());
    }

    #[test]
    fn encodings_of_small_multiples_of_basepoint() {
        // Table of encodings of i*basepoint
        // Generated using ristretto.sage
        let compressed = [
            CompressedRistretto([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            CompressedRistretto([226, 242, 174, 10, 106, 188, 78, 113, 168, 132, 169, 97, 197, 0, 81, 95, 88, 227, 11, 106, 165, 130, 221, 141, 182, 166, 89, 69, 224, 141, 45, 118]),
            CompressedRistretto([106, 73, 50, 16, 247, 73, 156, 209, 127, 236, 181, 16, 174, 12, 234, 35, 161, 16, 232, 213, 185, 1, 248, 172, 173, 211, 9, 92, 115, 163, 185, 25]),
            CompressedRistretto([148, 116, 31, 93, 93, 82, 117, 94, 206, 79, 35, 240, 68, 238, 39, 213, 209, 234, 30, 43, 209, 150, 180, 98, 22, 107, 22, 21, 42, 157, 2, 89]),
            CompressedRistretto([218, 128, 134, 39, 115, 53, 139, 70, 111, 250, 223, 224, 179, 41, 58, 179, 217, 253, 83, 197, 234, 108, 149, 83, 88, 245, 104, 50, 45, 175, 106, 87]),
            CompressedRistretto([232, 130, 177, 49, 1, 107, 82, 193, 211, 51, 112, 128, 24, 124, 247, 104, 66, 62, 252, 203, 181, 23, 187, 73, 90, 184, 18, 196, 22, 15, 244, 78]),
            CompressedRistretto([246, 71, 70, 211, 201, 43, 19, 5, 14, 216, 216, 2, 54, 167, 240, 0, 124, 59, 63, 150, 47, 91, 167, 147, 209, 154, 96, 30, 187, 29, 244, 3]),
            CompressedRistretto([68, 245, 53, 32, 146, 110, 200, 31, 189, 90, 56, 120, 69, 190, 183, 223, 133, 169, 106, 36, 236, 225, 135, 56, 189, 207, 166, 167, 130, 42, 23, 109]),
            CompressedRistretto([144, 50, 147, 216, 242, 40, 126, 190, 16, 226, 55, 77, 193, 165, 62, 11, 200, 135, 229, 146, 105, 159, 2, 208, 119, 213, 38, 60, 221, 85, 96, 28]),
            CompressedRistretto([2, 98, 42, 206, 143, 115, 3, 163, 28, 175, 198, 63, 143, 196, 143, 220, 22, 225, 200, 200, 210, 52, 178, 240, 214, 104, 82, 130, 169, 7, 96, 49]),
            CompressedRistretto([32, 112, 111, 215, 136, 178, 114, 10, 30, 210, 165, 218, 212, 149, 43, 1, 244, 19, 188, 240, 231, 86, 77, 232, 205, 200, 22, 104, 158, 45, 185, 95]),
            CompressedRistretto([188, 232, 63, 139, 165, 221, 47, 165, 114, 134, 76, 36, 186, 24, 16, 249, 82, 43, 198, 0, 74, 254, 149, 135, 122, 199, 50, 65, 202, 253, 171, 66]),
            CompressedRistretto([228, 84, 158, 225, 107, 154, 160, 48, 153, 202, 32, 140, 103, 173, 175, 202, 250, 76, 63, 62, 78, 83, 3, 222, 96, 38, 227, 202, 143, 248, 68, 96]),
            CompressedRistretto([170, 82, 224, 0, 223, 46, 22, 245, 95, 177, 3, 47, 195, 59, 196, 39, 66, 218, 214, 189, 90, 143, 192, 190, 1, 103, 67, 108, 89, 72, 80, 31]),
            CompressedRistretto([70, 55, 107, 128, 244, 9, 178, 157, 194, 181, 246, 240, 197, 37, 145, 153, 8, 150, 229, 113, 111, 65, 71, 124, 211, 0, 133, 171, 127, 16, 48, 30]),
            CompressedRistretto([224, 196, 24, 247, 200, 217, 196, 205, 215, 57, 91, 147, 234, 18, 79, 58, 217, 144, 33, 187, 104, 29, 252, 51, 2, 169, 217, 154, 46, 83, 230, 78]),
        ];
        let mut bp = RistrettoPoint::identity();
        for i in 0..16 {
            assert_eq!(bp.compress(), compressed[i]);
            bp = &bp + &constants::RISTRETTO_BASEPOINT_POINT;
        }
    }

    #[test]
    fn four_torsion_basepoint() {
        let bp = constants::RISTRETTO_BASEPOINT_POINT;
        let bp_coset = bp.coset4();
        for i in 0..4 {
            assert_eq!(bp, RistrettoPoint(bp_coset[i]));
        }
    }

    #[test]
    #[cfg(feature="precomputed_tables")]
    fn four_torsion_random() {
        let mut rng = OsRng::new().unwrap();
        let B = &constants::RISTRETTO_BASEPOINT_TABLE;
        let P = B * &Scalar::random(&mut rng);
        let P_coset = P.coset4();
        for i in 0..4 {
            assert_eq!(P, RistrettoPoint(P_coset[i]));
        }
    }

    #[test]
    fn elligator_vs_ristretto_sage() {
        // Test vectors extracted from ristretto.sage.
        //
        // Notice that all of the byte sequences have bit 255 set to 0; this is because
        // ristretto.sage does not mask the high bit of a field element.  When the high bit is set,
        // the ristretto.sage elligator implementation gives different results, since it takes a
        // different field element as input.
        let bytes: [[u8;32]; 16] = [
            [184, 249, 135, 49, 253, 123, 89, 113, 67, 160, 6, 239, 7, 105, 211, 41, 192, 249, 185, 57, 9, 102, 70, 198, 15, 127, 7, 26, 160, 102, 134, 71],
            [229, 14, 241, 227, 75, 9, 118, 60, 128, 153, 226, 21, 183, 217, 91, 136, 98, 0, 231, 156, 124, 77, 82, 139, 142, 134, 164, 169, 169, 62, 250, 52],
            [115, 109, 36, 220, 180, 223, 99, 6, 204, 169, 19, 29, 169, 68, 84, 23, 21, 109, 189, 149, 127, 205, 91, 102, 172, 35, 112, 35, 134, 69, 186, 34],
            [16, 49, 96, 107, 171, 199, 164, 9, 129, 16, 64, 62, 241, 63, 132, 173, 209, 160, 112, 215, 105, 50, 157, 81, 253, 105, 1, 154, 229, 25, 120, 83],
            [156, 131, 161, 162, 236, 251, 5, 187, 167, 171, 17, 178, 148, 210, 90, 207, 86, 21, 79, 161, 167, 215, 234, 1, 136, 242, 182, 248, 38, 85, 79, 86],
            [251, 177, 124, 54, 18, 101, 75, 235, 245, 186, 19, 46, 133, 157, 229, 64, 10, 136, 181, 185, 78, 144, 254, 167, 137, 49, 107, 10, 61, 10, 21, 25],
            [232, 193, 20, 68, 240, 77, 186, 77, 183, 40, 44, 86, 150, 31, 198, 212, 76, 81, 3, 217, 197, 8, 126, 128, 126, 152, 164, 208, 153, 44, 189, 77],
            [173, 229, 149, 177, 37, 230, 30, 69, 61, 56, 172, 190, 219, 115, 167, 194, 71, 134, 59, 75, 28, 244, 118, 26, 162, 97, 64, 16, 15, 189, 30, 64],
            [106, 71, 61, 107, 250, 117, 42, 151, 91, 202, 212, 100, 52, 188, 190, 21, 125, 218, 31, 18, 253, 241, 160, 133, 57, 242, 3, 164, 189, 68, 111, 75],
            [112, 204, 182, 90, 220, 198, 120, 73, 173, 107, 193, 17, 227, 40, 162, 36, 150, 141, 235, 55, 172, 183, 12, 39, 194, 136, 43, 153, 244, 118, 91, 89],
            [111, 24, 203, 123, 254, 189, 11, 162, 51, 196, 163, 136, 204, 143, 10, 222, 33, 112, 81, 205, 34, 35, 8, 66, 90, 6, 164, 58, 170, 177, 34, 25],
            [225, 183, 30, 52, 236, 82, 6, 183, 109, 25, 227, 181, 25, 82, 41, 193, 80, 77, 161, 80, 242, 203, 79, 204, 136, 245, 131, 110, 237, 106, 3, 58],
            [207, 246, 38, 56, 30, 86, 176, 90, 27, 200, 61, 42, 221, 27, 56, 210, 79, 178, 189, 120, 68, 193, 120, 167, 77, 185, 53, 197, 124, 128, 191, 126],
            [1, 136, 215, 80, 240, 46, 63, 147, 16, 244, 230, 207, 82, 189, 74, 50, 106, 169, 138, 86, 30, 131, 214, 202, 166, 125, 251, 228, 98, 24, 36, 21],
            [210, 207, 228, 56, 155, 116, 207, 54, 84, 195, 251, 215, 249, 199, 116, 75, 109, 239, 196, 251, 194, 246, 252, 228, 70, 146, 156, 35, 25, 39, 241, 4],
            [34, 116, 123, 9, 8, 40, 93, 189, 9, 103, 57, 103, 66, 227, 3, 2, 157, 107, 134, 219, 202, 74, 230, 154, 78, 107, 219, 195, 214, 14, 84, 80],
        ];
        let encoded_images: [CompressedRistretto; 16] = [
            CompressedRistretto([176, 157, 237, 97, 66, 29, 140, 166, 168, 94, 26, 157, 212, 216, 229, 160, 195, 246, 232, 239, 169, 112, 63, 193, 64, 32, 152, 69, 11, 190, 246, 86]),
            CompressedRistretto([234, 141, 77, 203, 181, 225, 250, 74, 171, 62, 15, 118, 78, 212, 150, 19, 131, 14, 188, 238, 194, 244, 141, 138, 166, 162, 83, 122, 228, 201, 19, 26]),
            CompressedRistretto([232, 231, 51, 92, 5, 168, 80, 36, 173, 179, 104, 68, 186, 149, 68, 40, 140, 170, 27, 103, 99, 140, 21, 242, 43, 62, 250, 134, 208, 255, 61, 89]),
            CompressedRistretto([208, 120, 140, 129, 177, 179, 237, 159, 252, 160, 28, 13, 206, 5, 211, 241, 192, 218, 1, 97, 130, 241, 20, 169, 119, 46, 246, 29, 79, 80, 77, 84]),
            CompressedRistretto([202, 11, 236, 145, 58, 12, 181, 157, 209, 6, 213, 88, 75, 147, 11, 119, 191, 139, 47, 142, 33, 36, 153, 193, 223, 183, 178, 8, 205, 120, 248, 110]),
            CompressedRistretto([26, 66, 231, 67, 203, 175, 116, 130, 32, 136, 62, 253, 215, 46, 5, 214, 166, 248, 108, 237, 216, 71, 244, 173, 72, 133, 82, 6, 143, 240, 104, 41]),
            CompressedRistretto([40, 157, 102, 96, 201, 223, 200, 197, 150, 181, 106, 83, 103, 126, 143, 33, 145, 230, 78, 6, 171, 146, 210, 143, 112, 5, 245, 23, 183, 138, 18, 120]),
            CompressedRistretto([220, 37, 27, 203, 239, 196, 176, 131, 37, 66, 188, 243, 185, 250, 113, 23, 167, 211, 154, 243, 168, 215, 54, 171, 159, 36, 195, 81, 13, 150, 43, 43]),
            CompressedRistretto([232, 121, 176, 222, 183, 196, 159, 90, 238, 193, 105, 52, 101, 167, 244, 170, 121, 114, 196, 6, 67, 152, 80, 185, 221, 7, 83, 105, 176, 208, 224, 121]),
            CompressedRistretto([226, 181, 183, 52, 241, 163, 61, 179, 221, 207, 220, 73, 245, 242, 25, 236, 67, 84, 179, 222, 167, 62, 167, 182, 32, 9, 92, 30, 165, 127, 204, 68]),
            CompressedRistretto([226, 119, 16, 242, 200, 139, 240, 87, 11, 222, 92, 146, 156, 243, 46, 119, 65, 59, 1, 248, 92, 183, 50, 175, 87, 40, 206, 53, 208, 220, 148, 13]),
            CompressedRistretto([70, 240, 79, 112, 54, 157, 228, 146, 74, 122, 216, 88, 232, 62, 158, 13, 14, 146, 115, 117, 176, 222, 90, 225, 244, 23, 94, 190, 150, 7, 136, 96]),
            CompressedRistretto([22, 71, 241, 103, 45, 193, 195, 144, 183, 101, 154, 50, 39, 68, 49, 110, 51, 44, 62, 0, 229, 113, 72, 81, 168, 29, 73, 106, 102, 40, 132, 24]),
            CompressedRistretto([196, 133, 107, 11, 130, 105, 74, 33, 204, 171, 133, 221, 174, 193, 241, 36, 38, 179, 196, 107, 219, 185, 181, 253, 228, 47, 155, 42, 231, 73, 41, 78]),
            CompressedRistretto([58, 255, 225, 197, 115, 208, 160, 143, 39, 197, 82, 69, 143, 235, 92, 170, 74, 40, 57, 11, 171, 227, 26, 185, 217, 207, 90, 185, 197, 190, 35, 60]),
            CompressedRistretto([88, 43, 92, 118, 223, 136, 105, 145, 238, 186, 115, 8, 214, 112, 153, 253, 38, 108, 205, 230, 157, 130, 11, 66, 101, 85, 253, 110, 110, 14, 148, 112]),
        ];
        for i in 0..16 {
            let r_0 = FieldElement::from_bytes(&bytes[i]);
            let Q = RistrettoPoint::elligator_ristretto_flavour(&r_0);
            assert_eq!(Q.compress(), encoded_images[i]);
        }
    }

    #[test]
    #[cfg(feature="precomputed_tables")]
    fn random_roundtrip() {
        let mut rng = OsRng::new().unwrap();
        let B = &constants::RISTRETTO_BASEPOINT_TABLE;
        for _ in 0..100 {
            let P = B * &Scalar::random(&mut rng);
            let compressed_P = P.compress();
            let Q = compressed_P.decompress().unwrap();
            assert_eq!(P, Q);
        }
    }

    #[test]
    fn random_is_valid() {
        let mut rng = OsRng::new().unwrap();
        for _ in 0..100 {
            let P = RistrettoPoint::random(&mut rng);
            // Check that P is on the curve
            assert!(P.0.is_valid());
            // Check that P is in the image of the ristretto map
            P.compress();
        }
    }
}

#[cfg(all(test, feature = "bench"))]
mod bench {
    use rand::OsRng;
    use test::Bencher;

    use super::*;

    #[bench]
    #[cfg(feature="precomputed_tables")]
    fn decompression(b: &mut Bencher) {
        let mut rng = OsRng::new().unwrap();
        let B = &constants::RISTRETTO_BASEPOINT_TABLE;
        let P = B * &Scalar::random(&mut rng);
        let P_compressed = P.compress();
        b.iter(|| P_compressed.decompress().unwrap());
    }

    #[bench]
    #[cfg(feature="precomputed_tables")]
    fn compression(b: &mut Bencher) {
        let mut rng = OsRng::new().unwrap();
        let B = &constants::RISTRETTO_BASEPOINT_TABLE;
        let P = B * &Scalar::random(&mut rng);
        b.iter(|| P.compress());
    }
}
