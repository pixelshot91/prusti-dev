// © 2021, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::{
    syntax::{LinearResource, PCSPermission, PCSPermissionKind, TemporaryPlace},
    util::*,
};
use itertools::Itertools;
use prusti_interface::{utils::is_prefix, PrustiError};
use prusti_rustc_interface::{
    data_structures::{stable_map::FxHashMap, stable_set::FxHashSet},
    errors::MultiSpan,
    middle::{
        mir::{Body, Local, Place},
        ty::TyCtxt,
    },
};

struct PCSRepacker<'tcx> {
    packs: Vec<Place<'tcx>>,
    unpacks: Vec<Place<'tcx>>,
    packed_meet: FxHashSet<Place<'tcx>>,
}

fn unify_moves<'tcx>(
    a: FxHashSet<PCSPermission<'tcx>>,
    b: FxHashSet<PCSPermission<'tcx>>,
    mir: &Body<'tcx>,
    tcx: TyCtxt<'tcx>,
) -> EncodingResult<()> {
    let mut mir_problems: FxHashMap<
        (Local, PCSPermissionKind),
        (FxHashSet<Place<'tcx>>, FxHashSet<Place<'tcx>>),
    > = FxHashMap::default();

    // Checking temporaries is simple: They aren't allowed to pack/unpack
    //      so we just need to check that they have the same mutability
    let _tmp_problems: FxHashMap<TemporaryPlace, PCSPermissionKind> = FxHashMap::default();

    // Split the problem into independent parts
    for pcs_permission in a.clone().into_iter() {
        let permissionkind = pcs_permission.kind;
        match pcs_permission.target {
            LinearResource::Mir(place) => {
                let local = place.local.clone();
                let set_borrow = mir_problems
                    .entry((local, permissionkind))
                    .or_insert((FxHashSet::default(), FxHashSet::default()));
                (*set_borrow).0.insert(place.clone());
            }
            LinearResource::Tmp(_temp) => {
                todo!();
            }
        }
    }

    // TODO: DRY

    for pcs_permission in b.into_iter() {
        let permissionkind = pcs_permission.kind.clone();
        match pcs_permission.target {
            LinearResource::Mir(place) => {
                let local = place.local.clone();
                let set_borrow = mir_problems
                    .entry((local, permissionkind))
                    .or_insert((FxHashSet::default(), FxHashSet::default()));
                (*set_borrow).1.insert(place.clone());
            }
            LinearResource::Tmp(_temp) => {
                todo!();
            }
        }
    }

    let mut a_unpacks: Vec<Place<'tcx>> = Vec::default();
    let mut b_unpacks: Vec<Place<'tcx>> = Vec::default();

    // Iterate over subproblems (in any order)
    let mut mir_problem_iter = mir_problems.drain();
    while let Some(((_local, _kind), (mut set_rc_a, mut set_rc_b))) = mir_problem_iter.next() {
        loop {
            // Remove (mark?) elements which do not need to be considered.
            let mut intersection: FxHashSet<Place<'tcx>> = FxHashSet::default();
            for x in set_rc_a.intersection(&set_rc_b) {
                intersection.insert(x.clone());
            }
            for x in intersection.into_iter() {
                set_rc_a.remove(&x);
                set_rc_b.remove(&x);
            }

            // If no more elements in set, we are done (they're unified)
            if (set_rc_a.len() == 0) && (set_rc_b.len() == 0) {
                break;
            }

            let mut gen_a: FxHashSet<Place<'tcx>> = FxHashSet::default();
            let mut kill_a: FxHashSet<Place<'tcx>> = FxHashSet::default();
            let mut gen_b: FxHashSet<Place<'tcx>> = FxHashSet::default();
            let mut kill_b: FxHashSet<Place<'tcx>> = FxHashSet::default();
            if let Some((a, _)) = set_rc_a
                .iter()
                .cartesian_product(set_rc_b.iter())
                .filter(|(a, b)| is_prefix(**a, **b))
                .next()
            {
                // b is a prefix of a => a should get expanded
                gen_a = FxHashSet::from_iter(expand_place(*a, mir, tcx)?);
                kill_a = FxHashSet::from_iter([*a].into_iter());
                a_unpacks.push(*a);
            } else if let Some((_, b)) = set_rc_a
                .iter()
                .cartesian_product(set_rc_b.iter())
                .filter(|(a, b)| is_prefix(**b, **a))
                .next()
            {
                // a is a prefix of b => b should get expanded
                gen_b = FxHashSet::from_iter(expand_place(*b, mir, tcx)?);
                kill_b = FxHashSet::from_iter([*b].into_iter());
                b_unpacks.push(*b);
            } else {
                return Err(PrustiError::internal(
                    format!("could not unify pcs's"),
                    MultiSpan::new(),
                ));
            }

            for a in kill_a.iter() {
                set_rc_a.remove(a);
            }

            for a in gen_a.iter() {
                set_rc_a.insert(*a);
            }

            for b in kill_b.iter() {
                set_rc_b.remove(b);
            }

            for b in gen_b.iter() {
                set_rc_b.insert(*b);
            }
        }
    }

    let mut working_pcs: FxHashSet<Place<'tcx>> = a
        .iter()
        .map(|perm| {
            if let LinearResource::Mir(p) = perm.target {
                p
            } else {
                panic!();
            }
        })
        .collect();

    for p in a_unpacks.iter() {
        if !working_pcs.remove(p) {
            return Err(PrustiError::internal(
                format!("prusti generated an incoherent unpack"),
                MultiSpan::new(),
            ));
        }
        for p1 in expand_place(*p, mir, tcx)? {
            if !working_pcs.insert(p1) {
                return Err(PrustiError::internal(
                    format!("prusti generated an incoherent unpack"),
                    MultiSpan::new(),
                ));
            }
        }
    }

    for p in b_unpacks.iter().rev() {
        if !working_pcs.remove(p) {
            return Err(PrustiError::internal(
                format!("prusti generated an incoherent pack"),
                MultiSpan::new(),
            ));
        }
        for p1 in expand_place(*p, mir, tcx)? {
            if !working_pcs.insert(p1) {
                return Err(PrustiError::internal(
                    format!("prusti generated an incoherent pack"),
                    MultiSpan::new(),
                ));
            }
        }
    }

    // At this point, we can check that b is a subset of the computed PCS.

    return Ok(());
}
