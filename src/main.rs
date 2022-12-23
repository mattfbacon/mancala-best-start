#![deny(
	absolute_paths_not_starting_with_crate,
	future_incompatible,
	keyword_idents,
	macro_use_extern_crate,
	meta_variable_misuse,
	missing_abi,
	missing_copy_implementations,
	non_ascii_idents,
	nonstandard_style,
	noop_method_call,
	pointer_structural_match,
	private_in_public,
	rust_2018_idioms,
	unused_qualifications
)]
#![warn(clippy::pedantic)]
#![allow(clippy::let_underscore_drop)]
#![forbid(unsafe_code)]

use std::ops::{Index, IndexMut};

type Amount = u8;

#[derive(Debug, Clone, Copy)]
struct BoardSide {
	bins: [Amount; 6],
	mancala: Amount,
}

impl BoardSide {
	fn new(start_with: Amount) -> Self {
		Self {
			bins: [start_with; 6],
			mancala: 0,
		}
	}
}

impl Index<Bin> for BoardSide {
	type Output = Amount;

	fn index(&self, bin: Bin) -> &Self::Output {
		&self.bins[usize::from(bin as u8)]
	}
}

impl IndexMut<Bin> for BoardSide {
	fn index_mut(&mut self, bin: Bin) -> &mut Self::Output {
		&mut self.bins[usize::from(bin as u8)]
	}
}

#[derive(Debug, Clone, Copy)]
struct Board {
	sides: [BoardSide; 2],
}

#[derive(Debug, Clone, Copy)]
enum MoveResult {
	GoAgain,
	TurnOver,
}

#[derive(Debug, Clone, Copy)]
enum FlatIndexKind {
	MyBins(u8),
	MyMancala,
	TheirBins(u8),
}

#[derive(Debug, Clone, Copy)]
struct FlatIndex(u8);

impl From<Bin> for FlatIndex {
	fn from(bin: Bin) -> Self {
		Self(bin as u8)
	}
}

impl FlatIndex {
	fn step(&mut self) {
		self.0 += 1;
		self.0 %= 13;
	}

	fn opposite(self) -> Self {
		Self(12 - self.0)
	}

	fn kind(self) -> FlatIndexKind {
		match self.0 {
			side0 @ 0..=5 => FlatIndexKind::MyBins(side0),
			6 => FlatIndexKind::MyMancala,
			side1 @ 7..=12 => FlatIndexKind::TheirBins(side1 - 7),
			_ => unreachable!(),
		}
	}
}

macro_rules! impl_flat_index {
	($self:ident, $index:ident $(, $mutness:tt)?) => {
		match $index.kind() {
			FlatIndexKind::MyBins(idx) => &$($mutness)? $self.sides[0].bins[usize::from(idx)],
			FlatIndexKind::MyMancala => &$($mutness)? $self.sides[0].mancala,
			FlatIndexKind::TheirBins(idx) => &$($mutness)? $self.sides[1].bins[usize::from(idx)],
		}
	};
}

impl Index<FlatIndex> for Board {
	type Output = Amount;

	fn index(&self, index: FlatIndex) -> &Amount {
		impl_flat_index!(self, index)
	}
}

impl IndexMut<FlatIndex> for Board {
	fn index_mut(&mut self, index: FlatIndex) -> &mut Amount {
		impl_flat_index!(self, index, mut)
	}
}

impl Board {
	fn new(start_with: Amount) -> Self {
		Self {
			sides: [BoardSide::new(start_with); 2],
		}
	}

	fn make_move_(&mut self, index: impl Into<FlatIndex>) -> MoveResult {
		// within this function `sides[0]` is the "current side"

		let mut index = index.into();
		let mut in_hand = std::mem::take(&mut self[index]);
		while in_hand > 0 {
			index.step();
			self[index] += 1;
			in_hand -= 1;
		}
		match index.kind() {
			FlatIndexKind::TheirBins(..) | FlatIndexKind::MyBins(..) if self[index] > 1 => {
				// TCO 🤞
				self.make_move_(index)
			}
			FlatIndexKind::TheirBins(..) => MoveResult::TurnOver,
			FlatIndexKind::MyBins(..) => {
				self.sides[0].mancala += std::mem::take(&mut self[index.opposite()]);
				MoveResult::TurnOver
			}
			FlatIndexKind::MyMancala => MoveResult::GoAgain,
		}
	}

	fn make_move(&mut self, move_: Bin) -> Option<MoveResult> {
		if self[move_.into()] == 0 {
			None
		} else {
			Some(self.make_move_(move_))
		}
	}
}

#[derive(Debug, Clone, Copy)]
enum Bin {
	A,
	B,
	C,
	D,
	E,
	F,
}

impl TryFrom<u8> for Bin {
	type Error = ();

	fn try_from(v: u8) -> Result<Self, Self::Error> {
		Bin::ALL.get(usize::from(v)).copied().ok_or(())
	}
}

impl Bin {
	const ALL: [Self; 6] = [Self::A, Self::B, Self::C, Self::D, Self::E, Self::F];
}

#[derive(Debug)]
enum Tree {
	TurnOver(Board),
	Continue(Box<[Option<Self>; 6]>),
}

impl Tree {
	fn build(board: Board) -> Self {
		let results = Bin::ALL.map(|bin| {
			let mut board = board;
			let res = board.make_move(bin);
			res.map(|res| match res {
				MoveResult::GoAgain => Self::build(board),
				MoveResult::TurnOver => Tree::TurnOver(board),
			})
		});
		Tree::Continue(Box::new(results))
	}

	fn find_max_paths(&self) -> Vec<(Amount, Box<[Bin]>)> {
		fn helper(tree: &Tree, path_so_far: &mut Vec<Bin>, out: &mut Vec<(Amount, Box<[Bin]>)>) {
			match tree {
				Tree::TurnOver(board) => {
					out.push((board.sides[0].mancala, path_so_far.as_slice().into()));
				}
				Tree::Continue(move_results) => {
					for (bin, result) in move_results
						.iter()
						.enumerate()
						.filter_map(|(bin, result)| Some((bin, result.as_ref()?)))
					{
						let bin = Bin::try_from(u8::try_from(bin).unwrap()).unwrap();
						path_so_far.push(bin);
						helper(result, path_so_far, out);
						path_so_far.pop();
					}
				}
			}
		}

		let mut path_so_far = Vec::new();
		let mut out = Vec::new();
		helper(self, &mut path_so_far, &mut out);
		// order by higher amounts first, then by shorter paths first
		out.sort_by(|(amount_a, path_a), (amount_b, path_b)| {
			amount_a
				.cmp(amount_b)
				.reverse()
				.then_with(|| path_a.len().cmp(&path_b.len()))
		});
		out
	}
}

fn main() {
	let tree = Tree::build(Board::new(4));
	let paths = tree.find_max_paths();
	for (amount, path) in &paths[..std::cmp::min(10, paths.len())] {
		print!("{amount} via ");
		for bin in path.iter() {
			print!("{bin:?}");
		}
		println!();
	}
}
