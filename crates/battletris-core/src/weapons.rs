//! Weapon catalog, arsenal slots, and bazaar shopping economy.

/// Number of legacy arsenal slots.
pub const ARSENAL_SLOT_COUNT: usize = 10;

/// Stable weapon token ABI from `BTWeaponToken`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WeaponToken {
    /// The dreaded feared weird.
    FearedWeird = 0,
    /// Four-by-Four.
    FourByFour = 1,
    /// The Mad Hatter.
    Hatter = 2,
    /// Upbyside-down.
    Upbyside = 3,
    /// Fallout.
    FallOut = 4,
    /// Swap meet.
    Swap = 5,
    /// Lawyer's delite.
    Lawyers = 6,
    /// Rise up.
    RiseUp = 7,
    /// Flip out.
    FlipOut = 8,
    /// Speedy Gonzales.
    Speedy = 9,
    /// Missing Pieces.
    Missing = 10,
    /// Piece It Together.
    PieceIt = 11,
    /// The Blind Cleric.
    Blind = 12,
    /// Mondale '96.
    Mondale = 13,
    /// Keating Five.
    Keating = 14,
    /// Carter Years.
    Carter = 15,
    /// Reagan Era.
    Reagan = 16,
    /// William Ames.
    Ames = 17,
    /// Ace of Spies.
    Ace = 18,
    /// The Condor.
    Condor = 19,
    /// Have a Nice Day.
    NiceDay = 20,
    /// So Long.
    SoLong = 21,
    /// No Dice.
    NoDice = 22,
    /// Bug Report.
    Bug = 23,
    /// Bottle neck.
    Bottle = 24,
    /// Slide Denied.
    NoSlide = 25,
    /// Lazy Susan.
    Susan = 26,
    /// Meadow.
    Meadow = 27,
    /// Mirror Mirror.
    Mirror = 28,
    /// The Twilight Zone.
    Twilight = 29,
    /// Slick Willy.
    Slick = 30,
    /// Broken Record.
    Broken = 31,
    /// The Force.
    Force = 32,
    /// The Gimp.
    Gimp = 33,
}

impl WeaponToken {
    /// Returns the stable legacy token id.
    #[must_use]
    pub const fn legacy_id(self) -> u8 {
        self as u8
    }

    /// Returns a token from a stable legacy id.
    #[must_use]
    pub const fn from_legacy_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::FearedWeird),
            1 => Some(Self::FourByFour),
            2 => Some(Self::Hatter),
            3 => Some(Self::Upbyside),
            4 => Some(Self::FallOut),
            5 => Some(Self::Swap),
            6 => Some(Self::Lawyers),
            7 => Some(Self::RiseUp),
            8 => Some(Self::FlipOut),
            9 => Some(Self::Speedy),
            10 => Some(Self::Missing),
            11 => Some(Self::PieceIt),
            12 => Some(Self::Blind),
            13 => Some(Self::Mondale),
            14 => Some(Self::Keating),
            15 => Some(Self::Carter),
            16 => Some(Self::Reagan),
            17 => Some(Self::Ames),
            18 => Some(Self::Ace),
            19 => Some(Self::Condor),
            20 => Some(Self::NiceDay),
            21 => Some(Self::SoLong),
            22 => Some(Self::NoDice),
            23 => Some(Self::Bug),
            24 => Some(Self::Bottle),
            25 => Some(Self::NoSlide),
            26 => Some(Self::Susan),
            27 => Some(Self::Meadow),
            28 => Some(Self::Mirror),
            29 => Some(Self::Twilight),
            30 => Some(Self::Slick),
            31 => Some(Self::Broken),
            32 => Some(Self::Force),
            33 => Some(Self::Gimp),
            _ => None,
        }
    }
}

/// One immutable weapon catalog row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponSpec {
    /// Stable token.
    pub token: WeaponToken,
    /// Legacy token symbol.
    pub legacy_symbol: &'static str,
    /// Legacy display name.
    pub name: &'static str,
    /// Legacy weapon description.
    pub description: &'static str,
    /// Base bazaar price.
    pub price: i32,
    /// Duration in target-player line clears; zero means one-shot.
    pub line_duration: u32,
}

/// Original catalog in stable `BTWeaponToken` order.
pub const WEAPON_CATALOG: [WeaponSpec; 34] = [
    spec(WeaponToken::FearedWeird, "BT_FEARED_WEIRD", "The Feared Weird", "Gives your opponent bizarre, disjointed pieces. None of the pieces are easily placed; particularly deadly when used in conjunction with either the Mad Hatter or No Dice.", 400, 3),
    spec(WeaponToken::FourByFour, "BT_FOUR_BY_FOUR", "Four-by-Four", "Evil incarnate. Replaces your opponent's box piece with one that is a four block by four block hollow box.", 425, 10),
    spec(WeaponToken::Hatter, "BT_HATTER", "The Mad Hatter", "Your opponent's pieces never stop spinning (unless pinned up against the wall). Quickly frustrates opponent. Very good combination weapon.", 375, 5),
    spec(WeaponToken::Upbyside, "BT_UPBYSIDE", "Upbyside-down", "Flips your opponent's screen upside-down. Their direction keys are reversed, and pieces rotate the opposite way.", 125, 10),
    spec(WeaponToken::FallOut, "BT_FALL_OUT", "Fallout", "The middle six columns of your board \"fall out.\" The gap that they leave represents a black hole. Any pieces dropped into this black hole will just disappear. The player must instead build a \"bridge\" of pieces over the hole in order to get lines.", 250, 10),
    spec(WeaponToken::Swap, "BT_SWAP", "Swap meet", "Swaps your screen with your opponent's. Screw up your own board, and then swap it out. Of course, the other opponent may launch another Swap, but such is life.", 1200, 0),
    spec(WeaponToken::Lawyers, "BT_LAWYERS", "Lawyer's delite", "Outright stolen from the original 2-player arcade version of tetris. Every line you get, your opponent's screen \"rises\" up by one line.", 350, 5),
    spec(WeaponToken::RiseUp, "BT_RISE_UP", "Rise up", "Raises the opponent's screen one level (the bottom level will be solid with one, random, block missing).", 75, 0),
    spec(WeaponToken::FlipOut, "BT_FLIP_OUT", "Flip out", "Flips your opponents screen on a vertical axis.  Can be extremely annoying if done often enough.", 15, 0),
    spec(WeaponToken::Speedy, "BT_SPEEDY", "Speedy Gonzales", "Doubles the speed of the opponent's game. Several of these launched at once get make things pretty interesting for your opponent.", 275, 10),
    spec(WeaponToken::Missing, "BT_MISSING", "Missing Pieces", "Randomly removes one of your opponent's blocks.", 50, 0),
    spec(WeaponToken::PieceIt, "BT_PIECE_IT", "Piece It Together", "Randomly adds a piece to your opponent's board. More than one great player has fallen on a lucky Piece It Together.", 100, 0),
    spec(WeaponToken::Blind, "BT_BLIND", "The Blind Cleric", "Bombs a region of your opponent's screen. Can be particularly annoying when an elaborate setup develops a large hole in its center.", 400, 0),
    spec(WeaponToken::Mondale, "BT_MONDALE", "Mondale '96", "Taxes your opponent with a hefty 30 percent rate. Whenever they get funds, you swipe a certain percentage.", 150, 50),
    spec(WeaponToken::Keating, "BT_KEATING", "Keating Five", "Your opponent's funds are all taken away...and given to you.", 425, 0),
    spec(WeaponToken::Carter, "BT_CARTER", "Carter Years", "Relives the inflationary years of Jimmy Carter -- the prices double at your opponent's bazaar.", 250, 20),
    spec(WeaponToken::Reagan, "BT_REAGAN", "Reagan Era", "Relives that era of debt -- your opponent's funds are multiplied by -1.", 425, 0),
    spec(WeaponToken::Ames, "BT_AMES", "William Ames", "Displays your opponent's screen and your opponent's funds next to your own.  Remember that cheap spies are easily bought and sold...", 50, 20),
    spec(WeaponToken::Ace, "BT_ACE", "Ace of Spies", "Send Reilly over the border.  A more expensive spy, but such is the price of greater accuracy.  Reilly's still human though...you never know when he's going to flake out on the Russian border.", 100, 30),
    spec(WeaponToken::Condor, "BT_CONDOR", "The Condor", "Launch the world's most advanced spy satellite.  Guaranteed accuracy, but you probably had to sell arms to the Contras in order to afford it.", 225, 40),
    spec(WeaponToken::NiceDay, "BT_NICE_DAY", "Have a Nice Day", "Gives your opponent a smiley face. Why give your opponent the opportunity to make an extra 150 beans? Hit them with a Reagan Era shortly after. God Bless America.", 50, 0),
    spec(WeaponToken::SoLong, "BT_SO_LONG", "So Long", "Deprives your opponent of long pieces.", 100, 10),
    spec(WeaponToken::NoDice, "BT_NO_DICE", "No Dice", "Deprives your opponent of dice.", 600, 35),
    spec(WeaponToken::Bug, "BT_BUG", "Bug Report", "Like Piece It Together, except the block is invisible (which leads your opponent to file a bug report).", 320, 0),
    spec(WeaponToken::Bottle, "BT_BOTTLE", "Bottle neck", "Your opponent's board suddenly develops a 4-block wide bottle neck.", 150, 10),
    spec(WeaponToken::NoSlide, "BT_NO_SLIDE", "Slide Denied", "Take the famous BattleTris slide out of your opponent's diet.", 125, 10),
    spec(WeaponToken::Susan, "BT_SUSAN", "Lazy Susan", "Turns the tables on you opponent by swapping your arsenal with theirs.", 600, 0),
    spec(WeaponToken::Meadow, "BT_MEADOW", "Meadow", "This weapon lineup simulates Meadow running on your opponent's machine:  the drop speed of their pieces is halved.", 475, 10),
    spec(WeaponToken::Mirror, "BT_MIRROR", "Mirror Mirror", "An oft requested defensive weapon:  when launched, your opponent's weapons will be reflected back on to them.  Note that some weapons (Swap Meet, Keating Five, Have a Nice Day, etc.) are simply nullified.", 500, 10),
    spec(WeaponToken::Twilight, "BT_TWILIGHT", "The Twilight Zone", "All of the bricks in your opponents screen becomes invisible.", 450, 0),
    spec(WeaponToken::Slick, "BT_SLICK", "Slick Willy", "Your opponent's pieces move endlessly from left to right and back.", 650, 3),
    spec(WeaponToken::Broken, "BT_BROKEN", "Broken Record", "Gives your opponent the same piece the same piece the same piece ...", 325, 5),
    spec(WeaponToken::Force, "BT_FORCE", "The Force", "When your opponent gets a line, his board won't drop to fill the empty space.", 325, 5),
    spec(WeaponToken::Gimp, "BT_GIMP", "The Gimp", "Distracts your opponent from the game.", 25, 0),
];

const fn spec(
    token: WeaponToken,
    legacy_symbol: &'static str,
    name: &'static str,
    description: &'static str,
    price: i32,
    line_duration: u32,
) -> WeaponSpec {
    WeaponSpec {
        token,
        legacy_symbol,
        name,
        description,
        price,
        line_duration,
    }
}

/// Returns the catalog row for a token.
#[must_use]
pub fn weapon_spec(token: WeaponToken) -> &'static WeaponSpec {
    &WEAPON_CATALOG[token.legacy_id() as usize]
}

/// Runtime state for line-duration weapons affecting one player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveEffects {
    remaining_lines: [u32; WEAPON_CATALOG.len()],
}

impl Default for ActiveEffects {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveEffects {
    /// Creates an empty active-effect set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            remaining_lines: [0; WEAPON_CATALOG.len()],
        }
    }

    /// Returns remaining target-player line clears for a token.
    #[must_use]
    pub const fn remaining_lines(&self, token: WeaponToken) -> u32 {
        self.remaining_lines[token.legacy_id() as usize]
    }

    /// Returns true if the token has remaining duration.
    #[must_use]
    pub const fn is_active(&self, token: WeaponToken) -> bool {
        self.remaining_lines(token) > 0
    }

    /// Adds a launch duration to any remaining duration, matching legacy stacking.
    pub fn activate(&mut self, token: WeaponToken) -> u32 {
        let duration = weapon_spec(token).line_duration;
        let slot = &mut self.remaining_lines[token.legacy_id() as usize];
        *slot = slot.saturating_add(duration);
        *slot
    }

    /// Decrements timed effects by cleared target-player lines and returns expirations.
    pub fn observe_line_clear(&mut self, lines: u32) -> Vec<WeaponToken> {
        if lines == 0 {
            return Vec::new();
        }

        let mut expired = Vec::new();
        for spec in WEAPON_CATALOG {
            let slot = &mut self.remaining_lines[spec.token.legacy_id() as usize];
            if *slot == 0 {
                continue;
            }
            *slot = slot.saturating_sub(lines);
            if *slot == 0 {
                expired.push(spec.token);
            }
        }
        expired
    }

    /// Decrements only selected timed effects by cleared lines and returns expirations.
    pub fn observe_line_clear_for(
        &mut self,
        lines: u32,
        tokens: impl IntoIterator<Item = WeaponToken>,
    ) -> Vec<WeaponToken> {
        if lines == 0 {
            return Vec::new();
        }

        let mut expired = Vec::new();
        for token in tokens {
            let slot = &mut self.remaining_lines[token.legacy_id() as usize];
            if *slot == 0 {
                continue;
            }
            *slot = slot.saturating_sub(lines);
            if *slot == 0 {
                expired.push(token);
            }
        }
        expired
    }
}

/// Returns whether a token has line duration and belongs to the Phase 9 timed set.
#[must_use]
pub const fn is_timed_weapon(token: WeaponToken) -> bool {
    matches!(
        token,
        WeaponToken::FearedWeird
            | WeaponToken::FourByFour
            | WeaponToken::Hatter
            | WeaponToken::Upbyside
            | WeaponToken::FallOut
            | WeaponToken::Lawyers
            | WeaponToken::Speedy
            | WeaponToken::Mondale
            | WeaponToken::Carter
            | WeaponToken::SoLong
            | WeaponToken::NoDice
            | WeaponToken::Bottle
            | WeaponToken::NoSlide
            | WeaponToken::Meadow
            | WeaponToken::Slick
            | WeaponToken::Broken
            | WeaponToken::Force
    )
}

/// Returns whether a token is one of the Phase 10 line-duration weapons.
#[must_use]
pub const fn is_phase_10_timed_weapon(token: WeaponToken) -> bool {
    matches!(
        token,
        WeaponToken::Ames | WeaponToken::Ace | WeaponToken::Condor | WeaponToken::Mirror
    )
}

/// Returns whether Mirror nullifies this launch instead of reflecting it.
#[must_use]
pub const fn mirror_nullifies(token: WeaponToken) -> bool {
    matches!(
        token,
        WeaponToken::Swap
            | WeaponToken::Mondale
            | WeaponToken::Keating
            | WeaponToken::Ames
            | WeaponToken::Ace
            | WeaponToken::Condor
            | WeaponToken::NiceDay
            | WeaponToken::Susan
            | WeaponToken::Mirror
    )
}

/// One occupied arsenal slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArsenalSlot {
    /// Weapon in this slot.
    pub token: WeaponToken,
    /// Stacked quantity in this slot.
    pub quantity: u32,
}

/// Ten-slot legacy arsenal. Empty slots are preserved as holes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arsenal {
    slots: [Option<ArsenalSlot>; ARSENAL_SLOT_COUNT],
}

impl Default for Arsenal {
    fn default() -> Self {
        Self::new()
    }
}

impl Arsenal {
    /// Creates an empty arsenal.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [None; ARSENAL_SLOT_COUNT],
        }
    }

    /// Returns all slots, preserving holes.
    #[must_use]
    pub const fn slots(&self) -> &[Option<ArsenalSlot>; ARSENAL_SLOT_COUNT] {
        &self.slots
    }

    /// Maps a number-key label to a zero-based arsenal slot. Label `0` means slot 10.
    #[must_use]
    pub const fn slot_index_for_label(label: u8) -> Option<usize> {
        match label {
            1..=9 => Some((label - 1) as usize),
            0 => Some(9),
            _ => None,
        }
    }

    /// Buys one weapon using legacy first-empty-or-earlier-stack semantics.
    pub fn buy_weapon(&mut self, token: WeaponToken) -> Result<usize, ArsenalError> {
        for (index, slot) in self.slots.iter_mut().enumerate() {
            match slot {
                Some(existing) if existing.token == token => {
                    existing.quantity += 1;
                    return Ok(index);
                }
                None => {
                    *slot = Some(ArsenalSlot { token, quantity: 1 });
                    return Ok(index);
                }
                Some(_) => {}
            }
        }

        Err(ArsenalError::Full)
    }

    /// Consumes one quantity from a slot, leaving a hole when it reaches zero.
    pub fn consume_slot_label(&mut self, label: u8) -> Result<WeaponToken, ArsenalError> {
        let index =
            Self::slot_index_for_label(label).ok_or(ArsenalError::InvalidSlotLabel(label))?;
        self.consume_slot(index)
    }

    /// Returns the weapon in a number-key slot without consuming it.
    pub fn token_for_slot_label(&self, label: u8) -> Result<WeaponToken, ArsenalError> {
        let index =
            Self::slot_index_for_label(label).ok_or(ArsenalError::InvalidSlotLabel(label))?;
        self.slots[index]
            .map(|slot| slot.token)
            .ok_or(ArsenalError::EmptySlot(index))
    }

    fn consume_slot(&mut self, index: usize) -> Result<WeaponToken, ArsenalError> {
        let slot = self.slots[index]
            .as_mut()
            .ok_or(ArsenalError::EmptySlot(index))?;
        let token = slot.token;
        slot.quantity -= 1;
        if slot.quantity == 0 {
            self.slots[index] = None;
        }
        Ok(token)
    }

    fn remove_one_staged(&mut self, index: usize, token: WeaponToken) -> Result<(), ArsenalError> {
        let slot = self.slots[index]
            .as_mut()
            .ok_or(ArsenalError::EmptySlot(index))?;
        if slot.token != token {
            return Err(ArsenalError::SlotTokenMismatch {
                index,
                expected: token,
                actual: slot.token,
            });
        }

        slot.quantity -= 1;
        if slot.quantity == 0 {
            self.slots[index] = None;
        }
        Ok(())
    }
}

/// Arsenal mutation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArsenalError {
    /// No empty slot and no earlier stack was available.
    Full,
    /// Slot key was not a legacy arsenal label.
    InvalidSlotLabel(u8),
    /// Slot was empty.
    EmptySlot(usize),
    /// Staged removal no longer matched the expected slot content.
    SlotTokenMismatch {
        /// Slot index.
        index: usize,
        /// Expected token.
        expected: WeaponToken,
        /// Actual token.
        actual: WeaponToken,
    },
}

/// Bazaar staging session. Purchases affect only staged funds/arsenal until committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bazaar {
    staged_arsenal: Arsenal,
    staged_funds: i32,
    carter_prices: bool,
    purchases: Vec<StagedPurchase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StagedPurchase {
    slot_index: usize,
    token: WeaponToken,
    price: i32,
}

impl Bazaar {
    /// Opens a bazaar session with entry-time Carter price capture.
    #[must_use]
    pub const fn new(arsenal: Arsenal, funds: i32, carter_prices: bool) -> Self {
        Self {
            staged_arsenal: arsenal,
            staged_funds: funds,
            carter_prices,
            purchases: Vec::new(),
        }
    }

    /// Returns staged funds after uncommitted purchases/removals.
    #[must_use]
    pub const fn staged_funds(&self) -> i32 {
        self.staged_funds
    }

    /// Returns staged arsenal after uncommitted purchases/removals.
    #[must_use]
    pub const fn staged_arsenal(&self) -> &Arsenal {
        &self.staged_arsenal
    }

    /// Returns whether Carter doubled prices for this whole bazaar entry.
    #[must_use]
    pub const fn carter_prices(&self) -> bool {
        self.carter_prices
    }

    /// Returns the entry-captured price for a token.
    #[must_use]
    pub fn price(&self, token: WeaponToken) -> i32 {
        let base = weapon_spec(token).price;
        if self.carter_prices {
            base * 2
        } else {
            base
        }
    }

    /// Stages one purchase if funds and arsenal capacity allow it.
    pub fn buy(&mut self, token: WeaponToken) -> Result<usize, BazaarError> {
        let price = self.price(token);
        if self.staged_funds < price {
            return Err(BazaarError::InsufficientFunds {
                available: self.staged_funds,
                required: price,
            });
        }

        let slot_index = self.staged_arsenal.buy_weapon(token)?;
        self.staged_funds -= price;
        self.purchases.push(StagedPurchase {
            slot_index,
            token,
            price,
        });
        Ok(slot_index)
    }

    /// Removes the most recent staged purchase for a token and refunds its captured price.
    pub fn remove_staged(&mut self, token: WeaponToken) -> Result<(), BazaarError> {
        let purchase_index = self
            .purchases
            .iter()
            .rposition(|purchase| purchase.token == token)
            .ok_or(BazaarError::NoStagedPurchase(token))?;
        let purchase = self.purchases.remove(purchase_index);
        self.staged_arsenal
            .remove_one_staged(purchase.slot_index, purchase.token)?;
        self.staged_funds += purchase.price;
        Ok(())
    }

    /// Commits staged funds and arsenal.
    #[must_use]
    pub fn commit(self) -> BazaarCommit {
        BazaarCommit {
            arsenal: self.staged_arsenal,
            funds: self.staged_funds,
        }
    }
}

/// Committed bazaar economy state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BazaarCommit {
    /// Committed arsenal.
    pub arsenal: Arsenal,
    /// Committed funds.
    pub funds: i32,
}

/// Bazaar shopping failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BazaarError {
    /// Arsenal mutation failed.
    Arsenal(ArsenalError),
    /// Not enough funds were available.
    InsufficientFunds {
        /// Available staged funds.
        available: i32,
        /// Required staged price.
        required: i32,
    },
    /// No newly staged purchase for this weapon can be removed.
    NoStagedPurchase(WeaponToken),
}

impl From<ArsenalError> for BazaarError {
    fn from(value: ArsenalError) -> Self {
        Self::Arsenal(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{Arsenal, ArsenalError, Bazaar, BazaarError, WeaponToken, WEAPON_CATALOG};

    #[test]
    fn catalog_preserves_legacy_order_prices_and_durations() {
        assert_eq!(WEAPON_CATALOG.len(), 34);
        for (index, spec) in WEAPON_CATALOG.iter().enumerate() {
            assert_eq!(spec.token.legacy_id(), index as u8);
            assert_eq!(WeaponToken::from_legacy_id(index as u8), Some(spec.token));
            assert!(!spec.name.is_empty());
            assert!(!spec.description.is_empty());
        }

        let rows = WEAPON_CATALOG.map(|spec| {
            (
                spec.legacy_symbol,
                spec.name,
                spec.price,
                spec.line_duration,
            )
        });
        assert_eq!(rows[0], ("BT_FEARED_WEIRD", "The Feared Weird", 400, 3));
        assert_eq!(rows[5], ("BT_SWAP", "Swap meet", 1200, 0));
        assert_eq!(rows[15], ("BT_CARTER", "Carter Years", 250, 20));
        assert_eq!(rows[28], ("BT_MIRROR", "Mirror Mirror", 500, 10));
        assert_eq!(rows[33], ("BT_GIMP", "The Gimp", 25, 0));
    }

    #[test]
    fn arsenal_number_labels_and_launch_consumption_preserve_holes() {
        assert_eq!(Arsenal::slot_index_for_label(1), Some(0));
        assert_eq!(Arsenal::slot_index_for_label(9), Some(8));
        assert_eq!(Arsenal::slot_index_for_label(0), Some(9));
        assert_eq!(Arsenal::slot_index_for_label(10), None);

        let mut arsenal = Arsenal::new();
        arsenal.buy_weapon(WeaponToken::Gimp).unwrap();
        assert_eq!(arsenal.consume_slot_label(1), Ok(WeaponToken::Gimp));
        assert_eq!(arsenal.slots()[0], None);
        assert_eq!(
            arsenal.consume_slot_label(1),
            Err(ArsenalError::EmptySlot(0))
        );
    }

    #[test]
    fn arsenal_stacks_only_before_the_first_hole_like_legacy() {
        let mut arsenal = Arsenal::new();
        assert_eq!(arsenal.buy_weapon(WeaponToken::Gimp), Ok(0));
        assert_eq!(arsenal.buy_weapon(WeaponToken::Force), Ok(1));
        assert_eq!(arsenal.buy_weapon(WeaponToken::Gimp), Ok(0));
        assert_eq!(arsenal.slots()[0].unwrap().quantity, 2);

        assert_eq!(arsenal.consume_slot_label(1), Ok(WeaponToken::Gimp));
        assert_eq!(arsenal.consume_slot_label(1), Ok(WeaponToken::Gimp));
        assert_eq!(arsenal.slots()[0], None);

        assert_eq!(arsenal.buy_weapon(WeaponToken::Force), Ok(0));
        assert_eq!(arsenal.slots()[0].unwrap().token, WeaponToken::Force);
        assert_eq!(arsenal.slots()[1].unwrap().token, WeaponToken::Force);
    }

    #[test]
    fn bazaar_stages_purchases_refunds_only_new_quantities_and_commits() {
        let mut arsenal = Arsenal::new();
        arsenal.buy_weapon(WeaponToken::Gimp).unwrap();
        let mut bazaar = Bazaar::new(arsenal, 100, false);

        assert_eq!(bazaar.buy(WeaponToken::Gimp), Ok(0));
        assert_eq!(bazaar.staged_funds(), 75);
        assert_eq!(bazaar.staged_arsenal().slots()[0].unwrap().quantity, 2);
        assert_eq!(bazaar.remove_staged(WeaponToken::Gimp), Ok(()));
        assert_eq!(bazaar.staged_funds(), 100);
        assert_eq!(bazaar.staged_arsenal().slots()[0].unwrap().quantity, 1);
        assert_eq!(
            bazaar.remove_staged(WeaponToken::Gimp),
            Err(BazaarError::NoStagedPurchase(WeaponToken::Gimp))
        );

        bazaar.buy(WeaponToken::FlipOut).unwrap();
        let commit = bazaar.commit();
        assert_eq!(commit.funds, 85);
        assert_eq!(
            commit.arsenal.slots()[1].unwrap().token,
            WeaponToken::FlipOut
        );
    }

    #[test]
    fn bazaar_captures_carter_prices_at_entry() {
        let mut bazaar = Bazaar::new(Arsenal::new(), 100, true);

        assert!(bazaar.carter_prices());
        assert_eq!(bazaar.price(WeaponToken::Gimp), 50);
        assert_eq!(bazaar.buy(WeaponToken::Gimp), Ok(0));
        assert_eq!(bazaar.staged_funds(), 50);
    }
}
