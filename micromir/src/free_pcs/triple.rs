// © 2023, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use prusti_rustc_interface::{
    data_structures::{fx::FxHashSet, graph::WithStartNode},
    dataflow::{storage, Analysis, ResultsCursor, AnalysisDomain, JoinSemiLattice, CallReturnPlaces,
        impls::{MaybeStorageLive, MaybeBorrowedLocals, MaybeRequiresStorage, MaybeLiveLocals}},
    index::vec::{Idx, IndexVec},
    middle::{
        mir::{visit::Visitor, Place,Rvalue, BorrowKind,
            Statement, StatementKind, TerminatorKind, Operand, Location, Terminator, Body, BasicBlock, HasLocalDecls, Local, RETURN_PLACE},
        ty::TyCtxt,
    },
    hir::Mutability,
};

use crate::{CapabilityKind, Fpcs};

// use super::update::UpdateSummary;

impl<'tcx> Visitor<'tcx> for Fpcs<'_, 'tcx> {
    fn visit_operand(&mut self,operand: &Operand<'tcx>,location:Location,){
        self.super_operand(operand,location);
        match *operand {
            Operand::Copy(place) => {
                self.requires_read(place);
            }
            Operand::Move(place) => {
                self.requires_exclusive(place);
                self.ensures_write(place);
            }
            Operand::Constant(..) => (),
        }
    }

    fn visit_statement(&mut self,statement: &Statement<'tcx>,location:Location,){
        self.super_statement(statement,location);
        use StatementKind::*;
        match &statement.kind {
            &Assign(box (place, _)) => {
                self.requires_write(place);
                self.ensures_exclusive(place);
            }
            &FakeRead(box (_, place)) => self.requires_read(place),
            &SetDiscriminant { box place, .. } => {
                self.requires_exclusive(place)
            }
            &Deinit(box place) => {
                // TODO: Maybe OK to also allow `Write` here?
                self.requires_exclusive(place);
                self.ensures_write(place);
            }
            &StorageLive(local) => {
                self.requires_unalloc(local);
                self.ensures_allocates(local);
            }
            &StorageDead(local) => {
                self.requires_unalloc_or_uninit(local);
                self.ensures_unalloc(local);
            }
            &Retag(_, box place) => {
                self.requires_exclusive(place)
            }
            AscribeUserType(..)
            | Coverage(..)
            | Intrinsic(..)
            | ConstEvalCounter
            | Nop => (),
        };
    }

    fn visit_terminator(&mut self,terminator: &Terminator<'tcx>,location:Location,){
        self.super_terminator(terminator,location);
        use TerminatorKind::*;
        match &terminator.kind {
            Goto { .. }
            | SwitchInt { .. }
            | Resume
            | Abort
            | Unreachable
            | Assert { .. }
            | GeneratorDrop
            | FalseEdge { .. }
            | FalseUnwind { .. } => (),
            Return => self.requires_exclusive(RETURN_PLACE),
            &Drop { place, .. } => {
                self.requires_write(place);
                self.ensures_write(place);
            }
            &DropAndReplace { place, .. } => {
                self.requires_write(place);
                self.ensures_exclusive(place);
            }
            &Call { destination, .. } => {
                self.requires_write(destination);
                self.ensures_exclusive(destination);
            }
            &Yield { resume_arg, .. } => {
                self.requires_write(resume_arg);
                self.ensures_exclusive(resume_arg);
            }
            InlineAsm { .. } => todo!("{terminator:?}"),
        };
    }

    fn visit_rvalue(&mut self,rvalue: &Rvalue<'tcx>,location:Location,){
        self.super_rvalue(rvalue,location);
        use Rvalue::*;
        match rvalue {
            Use(_) |
            Repeat(_, _) |
            ThreadLocalRef(_) |
            Cast(_, _, _) |
            BinaryOp(_, _) |
            CheckedBinaryOp(_, _) |
            NullaryOp(_, _) |
            UnaryOp(_, _) |
            Aggregate(_, _) => {}

            &Ref(_, bk, place) => match bk {
                BorrowKind::Shared => {
                    self.requires_read(place);
                    self.ensures_blocked_read(place);
                }
                // TODO: this should allow `Shallow Shared` as well
                BorrowKind::Shallow => {
                    self.requires_read(place);
                    self.ensures_blocked_read(place);
                }
                BorrowKind::Unique => {
                    self.requires_exclusive(place);
                    self.ensures_blocked_exclusive(place);
                }
                BorrowKind::Mut { .. } => {
                    self.requires_exclusive(place);
                    self.ensures_blocked_exclusive(place);
                }
            }
            &AddressOf(m, place) => match m {
                Mutability::Not => self.requires_read(place),
                Mutability::Mut => self.requires_exclusive(place),
            }
            &Len(place) => self.requires_read(place),
            &Discriminant(place) => self.requires_read(place),
            ShallowInitBox(_, _) => todo!(),
            &CopyForDeref(place) => self.requires_read(place),
        }
    }
}
