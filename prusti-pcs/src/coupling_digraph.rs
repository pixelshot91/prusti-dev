// © 2022, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
use crate::{
    hyperdigraph::{Bijection, DHEdge, Hyperdigraph},
    pcs::LoanSCC,
};
use analysis::mir_utils::{self, PlaceImpl};
use prusti_interface::environment::{
    borrowck::facts::{Loan, PointIndex},
    Environment,
};
use rustc_middle::{mir, ty::TyCtxt};
use std::{
    cmp::{self, Ordering},
    collections::BTreeSet,
    fmt::Debug,
    iter::zip,
};

// Custom wrapper for MIR places, used to implement
//  1. ordering
//  2. tagging

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct CPlace<'tcx>(pub mir::Place<'tcx>);

impl<'tcx> CPlace<'tcx> {
    pub fn cmp_local(&self, x: &CPlace<'tcx>) -> bool {
        self.0.local == x.0.local
    }

    /// Measure the lexicographic similarity between two place's projections
    pub fn cmp_lex(&self, x: &CPlace<'tcx>) -> u32 {
        let mut r: u32 = 0;
        for (p0, p1) in zip(self.0.iter_projections(), x.0.iter_projections()) {
            if p0 != p1 {
                break;
            }
            r += 1;
        }
        return r;
    }

    pub fn unpack(&self, mir: &mir::Body<'tcx>, tcx: TyCtxt<'tcx>) -> BTreeSet<Self> {
        mir_utils::expand_struct_place(self.0, mir, tcx, None)
            .iter()
            .map(|p| Self {
                0: p.to_mir_place(),
            })
            .collect()
    }
}

impl<'tcx> PartialOrd for CPlace<'tcx> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'tcx> Ord for CPlace<'tcx> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.0.local.cmp(&other.0.local) {
            Ordering::Equal => self.0.projection.cmp(other.0.projection),
            r @ (Ordering::Less | Ordering::Greater) => r,
        }
    }
}

/// A Coupling DAG is a HyperDAG of places, with edges annotated by
/// families of loans of which there exists (dynamically, at runtime)
/// a sequence of Viper statements to consume the LHS and produce the RHS.

#[derive(Default, Clone)]
pub struct CouplingDigraph<'tcx> {
    graph: Hyperdigraph<CPlace<'tcx>>,
    annotations: Bijection<DHEdge<CPlace<'tcx>>, BTreeSet<Loan>>,
}

/// Finds the place in a set with the same local, and most similar projections (lex).
fn closest_node_to<'a, 'tcx>(
    target: &CPlace<'tcx>,
    space: &'a BTreeSet<CPlace<'tcx>>,
) -> Option<&'a CPlace<'tcx>> {
    space
        .iter()
        .filter(|p| target.cmp_local(p))
        .map(|p| (p, target.cmp_lex(p)))
        .max_by(|(_, m), (_, n)| m.cmp(n))
        .map(|r| r.0)
}

impl<'tcx> CouplingDigraph<'tcx> {
    /// Add repack edge, returns a collection of the new nodes
    fn unpack_node<'mir>(
        &mut self,
        mir: &'mir mir::Body<'tcx>,
        tcx: TyCtxt<'tcx>,
        node: &CPlace<'tcx>,
    ) -> BTreeSet<CPlace<'tcx>> {
        let unpacking = node.unpack(mir, tcx);
        self.graph.include_edge(DHEdge {
            lhs: unpacking.clone(),
            rhs: BTreeSet::from([(*node).clone()]),
        });
        unpacking
    }

    /// Inserts repack edges into the graph so that "target" is reachable.
    /// Returns false if impossible (no places in the graph with the same base local)
    pub fn unpack_to_include<'mir>(
        &mut self,
        mir: &'mir mir::Body<'tcx>,
        tcx: TyCtxt<'tcx>,
        target: &CPlace<'tcx>,
    ) -> bool {
        let mut space: BTreeSet<CPlace<'tcx>> = (*self.graph.nodes()).clone();
        loop {
            let Some(closest) = closest_node_to(target, &space) else {
                return false;
            };
            if closest == target {
                return true;
            }
            space = self.unpack_node(mir, tcx, closest);
        }
    }

    pub fn new_loan<'mir>(
        &mut self,
        mir: &'mir mir::Body<'tcx>,
        tcx: TyCtxt<'tcx>,
        lhs: BTreeSet<CPlace<'tcx>>,
        rhs: BTreeSet<CPlace<'tcx>>,
        loan: Loan,
    ) {
        println!("\tissuing loan {:?}", loan);
        println!("\t\t lhs {:?}", lhs);
        println!("\t\t rhs {:?}", rhs);
        for l in rhs.iter() {
            // If possible, unpack existing nodes to include
            //  It's no problem if this can't be done under the eager packing condition,
            //  fixme: To be robust, we should use the general repacker here so that the
            //      eager packing condition is not needed.
            //  (example: coupled loans x.f -> _ and x.g -> _. Then an edge has LHS {x.f, x.g} -> _,
            //      which violates the eager packing condition and this algorithm
            //      unnecessarily fails if we reborrow from x. Not sure if this comes up
            //     until borrow in structs though.)
            let _ = self.unpack_to_include(mir, tcx, l);
        }
        let edge = DHEdge { lhs, rhs };
        self.graph.include_edge(edge.clone());
        self.annotations.insert(edge, BTreeSet::from([loan]))
    }

    /// This is going to be implemented slightly ad-hoc.
    /// After computing some examples I'll have a better idea of
    /// a general algorithm here.
    ///
    /// REQUIRES: the nodes of the LoanSCC to be sets of nodes in the
    /// hypergraph (kills and new loans are applied)
    /// NOTE: Possibly less live loans than edges in CDG
    pub fn constrain_by_scc(&mut self, scc: LoanSCC) {
        // If all origins either contain or do not contain a K-path,
        // quotient the graph by that K-path ("collapse" them).

        // "before" relation: ensure that
        println!("\tenter constraint algorithm");
        let distinguising_loans = scc.distinguishing_loans();
        for df in distinguising_loans.iter() {
            print!("\t\t[partition]: ");
            print_btree_inline(df);
        }

        // 1. Condense node constraints together

        // 2. Right-associate arrow constraints
        todo!();

        println!("\texit constraint algorithm");
    }

    pub fn pprint(&self) {
        self.graph.pprint_with_annotations(&self.annotations);
    }
}

fn print_btree_inline<T>(set: &BTreeSet<T>)
where
    T: Debug,
{
    print!("{{");
    for s in set.iter() {
        print!("{:?}, ", s);
    }
    println!("}}");
}
