//! Owned items and the bank.
//!
//! Everything a player owns sits behind one interface, `ItemStore`, so it can later move to
//! Steam's Inventory Service without the rest of the game changing:
//! - the wardrobe's cosmetics, dyes, titles and the vault's pet;
//! - the bank (custom tabs, a 28-slot carried inventory);
//! - gifts and admin grants.
//!
//! `LocalBank` is the store used today. It's kept on this PC in `bank_<SteamID>.dat`, with a
//! keyed SHA-256 checksum so a hand edit is noticed. A file that fails the check is set aside
//! and the last good copy is used instead.
//!
//! How things come to be owned:
//! - A look earned the usual way (the Rooftop Run, mastery, a game table, a vault drop) is put
//!   into the bank the first time it's earned. After that the bank is what counts, so a gifted
//!   piece leaves your wardrobe.
//! - Looks and dyes everyone starts with ("starter") are always yours and can't be gifted.

use crate::hub::{self, Earned, Unlock, LOOKS};
use crate::raid::{LootKind, LOOT};
use std::collections::{BTreeMap, BTreeSet};

/// One kind of item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKey {
    /// A wardrobe look (crate::hub::LOOKS id).
    Look(u8),
    /// A dye (crate::gear::DYES index).
    Dye(u8),
    /// A title (a crate::raid::LOOT id).
    Title(u8),
    /// A pet (1: the Cinder Wisp).
    Pet(u8),
}

/// What kind of thing it is (for the bank's sort and filters).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Cosmetic,
    Weapon,
    Dye,
    Title,
    Pet,
}

impl Category {
    pub fn name(self) -> &'static str {
        ["Cosmetic", "Weapon finish", "Dye", "Title", "Pet"][self as usize]
    }
}

impl ItemKey {
    pub fn to_code(self) -> String {
        match self {
            ItemKey::Look(v) => format!("L{v}"),
            ItemKey::Dye(v) => format!("D{v}"),
            ItemKey::Title(v) => format!("T{v}"),
            ItemKey::Pet(v) => format!("P{v}"),
        }
    }
    pub fn from_code(s: &str) -> Option<ItemKey> {
        let (k, v) = s.split_at_checked(1)?;
        let v: u8 = v.parse().ok()?;
        let key = match k {
            "L" => ItemKey::Look(v),
            "D" => ItemKey::Dye(v),
            "T" => ItemKey::Title(v),
            "P" => ItemKey::Pet(v),
            _ => return None,
        };
        key.valid().then_some(key)
    }
    /// For the network: kind then value.
    pub fn to_bytes(self) -> [u8; 2] {
        match self {
            ItemKey::Look(v) => [0, v],
            ItemKey::Dye(v) => [1, v],
            ItemKey::Title(v) => [2, v],
            ItemKey::Pet(v) => [3, v],
        }
    }
    pub fn from_bytes(b: [u8; 2]) -> Option<ItemKey> {
        let k = match b[0] {
            0 => ItemKey::Look(b[1]),
            1 => ItemKey::Dye(b[1]),
            2 => ItemKey::Title(b[1]),
            3 => ItemKey::Pet(b[1]),
            _ => return None,
        };
        k.valid().then_some(k)
    }
    /// It's a real item of the game.
    pub fn valid(self) -> bool {
        match self {
            ItemKey::Look(v) => hub::look(v).is_some(),
            ItemKey::Dye(v) => (v as usize) < crate::gear::DYES.len(),
            ItemKey::Title(v) => hub::title_text(v).is_some(),
            ItemKey::Pet(v) => v == 1,
        }
    }
    pub fn name(self) -> String {
        match self {
            ItemKey::Look(v) => hub::look(v).map_or("?".into(), |l| l.name.to_string()),
            ItemKey::Dye(v) => format!("{} dye", crate::gear::DYES.get(v as usize).map_or("?", |d| d.0)),
            ItemKey::Title(v) => format!("Title: {}", hub::title_text(v).unwrap_or("?")),
            ItemKey::Pet(_) => "Cinder Wisp (pet)".into(),
        }
    }
    pub fn category(self) -> Category {
        match self {
            ItemKey::Look(v) => {
                if hub::look(v).is_some_and(|l| matches!(l.slot, hub::Slot::Main | hub::Slot::Off)) {
                    Category::Weapon
                } else {
                    Category::Cosmetic
                }
            }
            ItemKey::Dye(_) => Category::Dye,
            ItemKey::Title(_) => Category::Title,
            ItemKey::Pet(_) => Category::Pet,
        }
    }
    /// Where it comes from (the bank's tooltip).
    pub fn source(self) -> String {
        match self {
            ItemKey::Look(v) => hub::look(v).map_or(String::new(), |l| format!("{} · {}", l.set, l.unlock.text())),
            ItemKey::Dye(v) => if crate::raid::loot_of_dye(v).is_some() { "found in the Ashen Vault".into() } else if v >= crate::gear::SEASON_DYES { "a season's reward".into() } else { "a starter dye".into() },
            _ => "found in the Ashen Vault".into(),
        }
    }
    /// Everyone has it from the start (it can't be gifted, and is never used up).
    pub fn starter(self) -> bool {
        match self {
            ItemKey::Look(v) => hub::look(v).is_some_and(|l| l.unlock == Unlock::Always),
            ItemKey::Dye(v) => crate::raid::loot_of_dye(v).is_none() && v < crate::gear::SEASON_DYES,
            _ => false,
        }
    }
    /// The raid loot id it is, if the vault gives it.
    pub fn loot(self) -> Option<u8> {
        match self {
            ItemKey::Look(v) => crate::raid::loot_of_look(v),
            ItemKey::Dye(v) => crate::raid::loot_of_dye(v),
            ItemKey::Title(v) => Some(v),
            ItemKey::Pet(_) => Some(crate::raid::L_PET),
        }
    }
    /// The item a vault drop is.
    pub fn of_loot(l: u8) -> Option<ItemKey> {
        Some(match LOOT.get(l as usize)?.kind {
            LootKind::Look(id) => ItemKey::Look(id),
            LootKind::Dye(d) => ItemKey::Dye(d),
            LootKind::Title => ItemKey::Title(l),
            LootKind::Pet => ItemKey::Pet(1),
        })
    }
}

/// Every item in the game, in catalogue order.
pub fn catalogue() -> Vec<ItemKey> {
    let mut v: Vec<ItemKey> = LOOKS.iter().map(|l| ItemKey::Look(l.id)).collect();
    v.extend((0..crate::gear::DYES.len() as u8).map(ItemKey::Dye));
    v.extend((0..LOOT.len() as u8).filter(|&l| LOOT[l as usize].kind == LootKind::Title).map(ItemKey::Title));
    v.push(ItemKey::Pet(1));
    v
}

/// Items whose name holds the words (any case), for ::item search and ::give.
pub fn search(q: &str) -> Vec<ItemKey> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    if let Some(k) = ItemKey::from_code(&q.to_uppercase()) {
        return vec![k];
    }
    catalogue().into_iter().filter(|k| k.name().to_lowercase().contains(&q)).collect()
}

/// The one item a name means: exactly that name, else the only one containing it.
pub fn find(q: &str) -> Result<ItemKey, String> {
    let all = search(q);
    let ql = q.trim().to_lowercase();
    if let Some(k) = all.iter().find(|k| k.name().to_lowercase() == ql) {
        return Ok(*k);
    }
    match all.len() {
        0 => Err(format!("no item called \"{}\" (try ::item search)", q.trim())),
        1 => Ok(all[0]),
        n => Err(format!("{n} items match \"{}\": {}", q.trim(), all.iter().take(4).map(|k| k.name()).collect::<Vec<_>>().join(", "))),
    }
}

/// Why something can't be taken out of a store.
#[derive(Clone, Debug, PartialEq)]
pub enum StoreError {
    NotOwned,
    Starter,
}

/// Owning items. Today: the bank on this PC. Later: Steam's Inventory Service.
pub trait ItemStore {
    /// How many of an item are owned (the bank and the carried inventory).
    fn count(&self, key: ItemKey) -> u32;
    /// Whether it can be worn or used.
    fn owns(&self, key: ItemKey) -> bool {
        key.starter() || self.count(key) > 0
    }
    /// Add items (earned, a gift, an admin's grant), with a note for the history.
    fn grant(&mut self, key: ItemKey, n: u32, why: &str);
    /// Take items out (a gift sent).
    fn consume(&mut self, key: ItemKey, n: u32, why: &str) -> Result<(), StoreError>;
    /// Everything owned, with counts.
    fn list(&self) -> Vec<(ItemKey, u32)>;
}

/// A tab in the bank: its name and the items filed under it (in order).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tab {
    pub name: String,
    pub items: Vec<ItemKey>,
}

pub const CARRY_SLOTS: usize = 28;
pub const MAX_TABS: usize = 9;

/// The bank: what's owned, where it's filed, what's carried, and what's happened to it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocalBank {
    /// In the bank (not carried): counts.
    pub items: BTreeMap<ItemKey, u32>,
    /// The 28 carried slots.
    pub carried: [Option<ItemKey>; CARRY_SLOTS],
    /// Looks and drops put in once when earned (so a gifted one isn't put back).
    pub granted: BTreeSet<ItemKey>,
    /// Custom tabs (the "All" view isn't one of them).
    pub tabs: Vec<Tab>,
    /// Items set aside for a gift waiting for its answer: (gift id, item, count).
    pub escrow: Vec<(u32, ItemKey, u32)>,
    /// Gifts and grants, newest last.
    pub history: Vec<String>,
    /// The progress the bank was made from (the save's copy can be edited; this one can't).
    pub earned: Earned,
    /// Bumped on every change (the menu side saves when it moves).
    pub version: u32,
}

impl ItemStore for LocalBank {
    fn count(&self, key: ItemKey) -> u32 {
        self.items.get(&key).copied().unwrap_or(0) + self.carried.iter().filter(|c| **c == Some(key)).count() as u32
    }
    fn grant(&mut self, key: ItemKey, n: u32, why: &str) {
        if n == 0 || !key.valid() {
            return;
        }
        *self.items.entry(key).or_insert(0) += n;
        self.note(format!("+{n} {} ({why})", key.name()));
    }
    fn consume(&mut self, key: ItemKey, n: u32, why: &str) -> Result<(), StoreError> {
        if key.starter() {
            return Err(StoreError::Starter);
        }
        if self.count(key) < n || n == 0 {
            return Err(StoreError::NotOwned);
        }
        // from the bank first, then the carried slots
        let mut left = n;
        let in_bank = self.items.get(&key).copied().unwrap_or(0);
        let take = in_bank.min(left);
        if take > 0 {
            if in_bank == take {
                self.items.remove(&key);
            } else {
                self.items.insert(key, in_bank - take);
            }
            left -= take;
        }
        for c in self.carried.iter_mut() {
            if left > 0 && *c == Some(key) {
                *c = None;
                left -= 1;
            }
        }
        self.note(format!("-{n} {} ({why})", key.name()));
        Ok(())
    }
    fn list(&self) -> Vec<(ItemKey, u32)> {
        let mut keys: BTreeSet<ItemKey> = self.items.keys().copied().collect();
        keys.extend(self.carried.iter().flatten().copied());
        keys.into_iter().map(|k| (k, self.count(k))).collect()
    }
}

impl LocalBank {
    fn note(&mut self, s: String) {
        self.version = self.version.wrapping_add(1);
        self.history.push(s);
        if self.history.len() > 200 {
            self.history.remove(0);
        }
    }

    /// Put in anything newly earned (once each).
    pub fn sync(&mut self, earned: &Earned) {
        // (the progress kept here only grows)
        let e = &mut self.earned;
        e.rooftop |= earned.rooftop;
        e.table_hard |= earned.table_hard;
        e.pit_wins = e.pit_wins.max(earned.pit_wins);
        e.mastery = e.mastery.max(earned.mastery);
        e.raid.items |= earned.raid.items;
        e.raid.cleared |= earned.raid.cleared;
        e.raid.cleared_hard |= earned.raid.cleared_hard;
        for k in 0..3 {
            if earned.raid.best[k] != 0 && (e.raid.best[k] == 0 || earned.raid.best[k] < e.raid.best[k]) {
                e.raid.best[k] = earned.raid.best[k];
            }
        }
        let e = self.earned;
        let mut due: Vec<ItemKey> = LOOKS.iter().filter(|l| l.unlock != Unlock::Always && e.has(l.unlock)).map(|l| ItemKey::Look(l.id)).collect();
        due.extend((0..LOOT.len() as u8).filter(|&l| e.raid.has(l)).filter_map(ItemKey::of_loot));
        for k in due {
            if !k.starter() && self.granted.insert(k) {
                self.grant(k, 1, "earned");
            }
        }
    }

    /// The summary the wardrobe's checks use (crate::hub::Earned).
    pub fn apply_to(&self, earned: &mut Earned) {
        earned.bank_on = true;
        earned.own_looks = 0;
        earned.own_loot = 0;
        for (k, _) in self.list() {
            match k {
                ItemKey::Look(v) if v < 128 => earned.own_looks |= 1u128 << v,
                _ => {}
            }
            if let Some(l) = k.loot() {
                if l < 64 && (matches!(k, ItemKey::Dye(_) | ItemKey::Title(_) | ItemKey::Pet(_))) {
                    earned.own_loot |= 1u64 << l;
                }
            }
        }
    }

    /// Everything (wardrobe items too): starters are shown as owned.
    pub fn everything(&self) -> Vec<(ItemKey, u32)> {
        let mut v = self.list();
        for k in catalogue() {
            if k.starter() && !v.iter().any(|x| x.0 == k) {
                v.push((k, 1));
            }
        }
        v
    }

    /// Take one into the first free carried slot.
    pub fn carry(&mut self, key: ItemKey) -> bool {
        let n = self.items.get(&key).copied().unwrap_or(0);
        let Some(slot) = self.carried.iter().position(|c| c.is_none()) else { return false };
        if n == 0 {
            return false;
        }
        if n == 1 {
            self.items.remove(&key);
        } else {
            self.items.insert(key, n - 1);
        }
        self.carried[slot] = Some(key);
        self.version = self.version.wrapping_add(1);
        true
    }
    /// Put a carried slot's item back in the bank.
    pub fn deposit(&mut self, slot: usize) -> bool {
        let Some(k) = self.carried.get_mut(slot).and_then(|c| c.take()) else { return false };
        *self.items.entry(k).or_insert(0) += 1;
        self.version = self.version.wrapping_add(1);
        true
    }
    /// Swap two carried slots (dragging in the inventory).
    pub fn swap_carried(&mut self, a: usize, b: usize) {
        if a < CARRY_SLOTS && b < CARRY_SLOTS {
            self.carried.swap(a, b);
            self.version = self.version.wrapping_add(1);
        }
    }
    /// A new tab (named), or None when there are already MAX_TABS.
    pub fn add_tab(&mut self, name: &str) -> Option<usize> {
        if self.tabs.len() >= MAX_TABS {
            return None;
        }
        self.tabs.push(Tab { name: clean_tab_name(name), items: Vec::new() });
        self.version = self.version.wrapping_add(1);
        Some(self.tabs.len() - 1)
    }
    pub fn rename_tab(&mut self, t: usize, name: &str) {
        if let Some(tab) = self.tabs.get_mut(t) {
            tab.name = clean_tab_name(name);
            self.version = self.version.wrapping_add(1);
        }
    }
    pub fn remove_tab(&mut self, t: usize) {
        if t < self.tabs.len() {
            self.tabs.remove(t);
            self.version = self.version.wrapping_add(1);
        }
    }
    /// File an item under a tab (None: no tab), at a place in it (dragging).
    pub fn file(&mut self, key: ItemKey, tab: Option<usize>, at: Option<usize>) {
        for t in self.tabs.iter_mut() {
            t.items.retain(|k| *k != key);
        }
        if let Some(t) = tab.and_then(|t| self.tabs.get_mut(t)) {
            let at = at.unwrap_or(t.items.len()).min(t.items.len());
            t.items.insert(at, key);
        }
        self.version = self.version.wrapping_add(1);
    }
    /// The tab an item is filed under.
    pub fn tab_of(&self, key: ItemKey) -> Option<usize> {
        self.tabs.iter().position(|t| t.items.contains(&key))
    }

    /// Set aside items for a gift (they come back if it's turned down).
    pub fn hold_for_gift(&mut self, gid: u32, key: ItemKey, n: u32) -> Result<(), StoreError> {
        self.consume(key, n, "set aside for a gift")?;
        self.escrow.push((gid, key, n));
        Ok(())
    }
    /// A gift went through (true) or didn't (its items come back).
    pub fn settle_gift(&mut self, gid: u32, sent: bool, to: &str) {
        let Some(i) = self.escrow.iter().position(|e| e.0 == gid) else { return };
        let (_, key, n) = self.escrow.remove(i);
        if sent {
            self.note(format!("gave {n} {} to {to}", key.name()));
        } else {
            *self.items.entry(key).or_insert(0) += n;
            self.note(format!("gift of {} to {to} didn't go through: back in the bank", key.name()));
        }
    }

    // ------------------------------------------------------------------ the file

    pub fn to_text(&self) -> String {
        let mut s = String::from("bank=1\n");
        s += &format!("items={}\n", self.items.iter().map(|(k, n)| format!("{}:{n}", k.to_code())).collect::<Vec<_>>().join(","));
        s += &format!("carried={}\n", self.carried.iter().map(|c| c.map_or("-".to_string(), |k| k.to_code())).collect::<Vec<_>>().join(","));
        s += &format!("granted={}\n", self.granted.iter().map(|k| k.to_code()).collect::<Vec<_>>().join(","));
        for t in &self.tabs {
            s += &format!("tab={}|{}\n", t.name, t.items.iter().map(|k| k.to_code()).collect::<Vec<_>>().join(","));
        }
        for (g, k, n) in &self.escrow {
            s += &format!("escrow={g}:{}:{n}\n", k.to_code());
        }
        let e = &self.earned;
        s += &format!("progress={},{},{},{}\n", e.rooftop as u8, e.table_hard as u8, e.pit_wins, e.mastery);
        s += &format!("raid={}\n", e.raid.to_text());
        for h in self.history.iter().rev().take(60).rev() {
            s += &format!("log={}\n", h.replace('\n', " "));
        }
        s
    }

    pub fn from_text(t: &str) -> LocalBank {
        let mut b = LocalBank::default();
        for line in t.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k {
                "items" => {
                    for p in v.split(',') {
                        if let Some((c, n)) = p.split_once(':') {
                            if let (Some(k), Ok(n)) = (ItemKey::from_code(c), n.parse::<u32>()) {
                                if n > 0 {
                                    b.items.insert(k, n.min(9999));
                                }
                            }
                        }
                    }
                }
                "carried" => {
                    for (i, c) in v.split(',').enumerate().take(CARRY_SLOTS) {
                        b.carried[i] = ItemKey::from_code(c);
                    }
                }
                "granted" => b.granted = v.split(',').filter_map(ItemKey::from_code).collect(),
                "tab" => {
                    if b.tabs.len() < MAX_TABS {
                        let (n, list) = v.split_once('|').unwrap_or((v, ""));
                        b.tabs.push(Tab { name: clean_tab_name(n), items: list.split(',').filter_map(ItemKey::from_code).collect() });
                    }
                }
                "escrow" => {
                    let p: Vec<&str> = v.split(':').collect();
                    if let (Some(g), Some(k), Some(n)) = (p.first().and_then(|x| x.parse().ok()), p.get(1).and_then(|x| ItemKey::from_code(x)), p.get(2).and_then(|x| x.parse().ok())) {
                        b.escrow.push((g, k, n));
                    }
                }
                "progress" => {
                    let p: Vec<u32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
                    b.earned.rooftop = p.first() == Some(&1);
                    b.earned.table_hard = p.get(1) == Some(&1);
                    b.earned.pit_wins = p.get(2).copied().unwrap_or(0);
                    b.earned.mastery = p.get(3).copied().unwrap_or(0);
                }
                "raid" => b.earned.raid = crate::raid::RaidEarned::from_text(v),
                "log" => b.history.push(v.to_string()),
                _ => {}
            }
        }
        b
    }
}

fn clean_tab_name(s: &str) -> String {
    let t: String = s.chars().filter(|c| !c.is_control() && *c != '|' && *c != ',').take(16).collect();
    if t.trim().is_empty() { "Tab".into() } else { t.trim().to_string() }
}

// ============================================================================ the file on disk

/// The key mixed into the checksum (with the account), so the sum can't simply be recomputed.
const BANK_KEY: &[u8] = b"DuelRush/bank/v1/\x8e\x14\x55\xd2\x03\x9a\xc1\x77\x2b\xe0\x6f\x41\x90\x3c\xa8\x5d";

fn sum(account: u64, body: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(BANK_KEY);
    h.update(account.to_le_bytes());
    h.update(body.as_bytes());
    h.update(BANK_KEY);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The bank as a file: its text, then its checksum.
pub fn seal(bank: &LocalBank, account: u64) -> String {
    let body = bank.to_text();
    let s = sum(account, &body);
    format!("{body}sum={s}\n")
}

/// Read a sealed bank: None when the checksum doesn't match (edited by hand, or someone else's).
pub fn unseal(text: &str, account: u64) -> Option<LocalBank> {
    let t = text.replace("\r\n", "\n");
    let at = t.rfind("sum=")?;
    let (body, tail) = t.split_at(at);
    let given = tail.trim_start_matches("sum=").trim();
    (given == sum(account, body)).then(|| LocalBank::from_text(body))
}

/// The file for an account (0: no Steam, "local").
pub fn bank_path(account: u64) -> Option<std::path::PathBuf> {
    crate::platform::data_file(&if account == 0 { "bank_local.dat".to_string() } else { format!("bank_{account}.dat") })
}

/// Load this account's bank: (the bank, a note when the file had been tampered with).
pub fn load(account: u64) -> (LocalBank, Option<String>) {
    let Some(p) = bank_path(account) else { return (LocalBank::default(), None) };
    load_at(&p, account)
}

/// Load a bank file (and its .bak beside it).
pub fn load_at(p: &std::path::Path, account: u64) -> (LocalBank, Option<String>) {
    let p = p.to_path_buf();
    let Ok(t) = std::fs::read_to_string(&p) else { return (LocalBank::default(), None) };
    if let Some(b) = unseal(&t, account) {
        return (b, None);
    }
    // edited by hand: set it aside, and use the last good copy
    let _ = std::fs::rename(&p, p.with_extension("tampered"));
    let back = std::fs::read_to_string(p.with_extension("bak")).ok().and_then(|t| unseal(&t, account));
    let note = "Your bank file had been changed outside the game, so it was set aside".to_string();
    match back {
        Some(b) => (b, Some(format!("{note} and the last good copy was loaded."))),
        None => (LocalBank::default(), Some(format!("{note}; there was no good copy to load."))),
    }
}

/// Save it (the previous good file kept as .bak).
pub fn save(bank: &LocalBank, account: u64) {
    let Some(p) = bank_path(account) else { return };
    save_at(bank, &p, account);
}

pub fn save_at(bank: &LocalBank, p: &std::path::Path, account: u64) {
    let p = p.to_path_buf();
    if p.exists() {
        let _ = std::fs::copy(&p, p.with_extension("bak"));
    }
    let _ = crate::platform::save_file(&p, &seal(bank, account));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sealed_bank_reads_back_and_an_edit_is_caught() {
        let mut b = LocalBank::default();
        b.grant(ItemKey::Look(84), 1, "test");
        b.grant(ItemKey::Dye(26), 2, "test");
        b.add_tab("Vault");
        b.file(ItemKey::Look(84), Some(0), None);
        assert!(b.carry(ItemKey::Dye(26)));
        b.earned.raid.give(25);
        let text = seal(&b, 777);
        let back = unseal(&text, 777).expect("reads back");
        assert_eq!(back.items, b.items);
        assert_eq!(back.carried, b.carried);
        assert_eq!(back.tabs, b.tabs);
        assert_eq!(back.earned.raid, b.earned.raid);
        assert_eq!(back.count(ItemKey::Dye(26)), 2);
        // hand-edited: one more coat
        let edited = text.replace("L84:1", "L84:5");
        assert_ne!(edited, text);
        assert!(unseal(&edited, 777).is_none(), "an edit is caught");
        // someone else's file
        assert!(unseal(&text, 778).is_none(), "another account's file doesn't load");
        // progress edited
        let e2 = text.replace("progress=0,0,0,0", "progress=1,1,99,99");
        assert!(e2 == text || unseal(&e2, 777).is_none());
    }

    #[test]
    fn earned_looks_go_in_once_and_a_gifted_one_stays_gone() {
        let mut b = LocalBank::default();
        let e = Earned { rooftop: true, ..Default::default() };
        b.sync(&e);
        let smug = LOOKS.iter().find(|l| l.unlock == Unlock::Rooftop).unwrap().id;
        assert_eq!(b.count(ItemKey::Look(smug)), 1);
        b.consume(ItemKey::Look(smug), 1, "gift").unwrap();
        b.sync(&e);
        assert_eq!(b.count(ItemKey::Look(smug)), 0, "not put back");
        let mut summary = Earned::default();
        b.apply_to(&mut summary);
        assert!(!summary.look_owned(hub::look(smug).unwrap()));
        assert!(summary.look_owned(hub::look(1).unwrap()), "starters always");
        // starters can't be given away
        assert_eq!(b.consume(ItemKey::Look(1), 1, "x"), Err(StoreError::Starter));
    }

    #[test]
    fn items_are_found_by_name_and_code() {
        assert_eq!(find("regent's long coat"), Ok(ItemKey::Look(84)));
        assert_eq!(find("L84"), Ok(ItemKey::Look(84)));
        assert!(find("zzz").is_err());
        assert!(search("vault warden").len() >= 7);
        for k in catalogue() {
            assert_eq!(ItemKey::from_code(&k.to_code()), Some(k));
            assert_eq!(ItemKey::from_bytes(k.to_bytes()), Some(k));
        }
    }
}
