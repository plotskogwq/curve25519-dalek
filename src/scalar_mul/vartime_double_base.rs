// -*- mode: rust; -*-
//
// This file is part of curve25519-dalek.
// Copyright (c) 2016-2018 Isis Lovecruft, Henry de Valence
// See LICENSE for licensing information.
//
// Authors:
// - Isis Agora Lovecruft <isis@patternsinthevoid.net>
// - Henry de Valence <hdevalence@hdevalence.ca>
#![allow(non_snake_case)]

use constants;
use traits::Identity;
use scalar::Scalar;
use edwards::EdwardsPoint;
use curve_models::{ProjectiveNielsPoint, ProjectivePoint};
use scalar_mul::window::OddLookupTable;

/// Compute \\(aA + bB\\) in variable time, where \\(B\\) is the Ed25519 basepoint.
pub fn mul(a: &Scalar, A: &EdwardsPoint, b: &Scalar) -> EdwardsPoint {
    let a_naf = a.non_adjacent_form();
    let b_naf = b.non_adjacent_form();

    // Find starting index
    let mut i: usize = 255;
    for j in (0..255).rev() {
        i = j;
        if a_naf[i] != 0 || b_naf[i] != 0 {
            break;
        }
    }

    let table_A = OddLookupTable::<ProjectiveNielsPoint>::from(A);
    let table_B = &constants::AFFINE_ODD_MULTIPLES_OF_BASEPOINT;

    let mut r = ProjectivePoint::identity();
    loop {
        let mut t = r.double();

        if a_naf[i] > 0 {
            t = &t.to_extended() + &table_A.select(a_naf[i] as usize);
        } else if a_naf[i] < 0 {
            t = &t.to_extended() - &table_A.select(-a_naf[i] as usize);
        }

        if b_naf[i] > 0 {
            t = &t.to_extended() + &table_B.select(b_naf[i] as usize);
        } else if b_naf[i] < 0 {
            t = &t.to_extended() - &table_B.select(-b_naf[i] as usize);
        }

        r = t.to_projective();

        if i == 0 {
            break;
        }
        i -= 1;
    }

    r.to_extended()
}
