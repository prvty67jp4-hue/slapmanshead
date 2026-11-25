//! Gifts between players.
//!
//! How a gift goes:
//! 1. The giver picks an item from their bank and sees exactly what moves before confirming.
//!    The item is set aside in the giver's bank and the offer goes to the host.
//! 2. The host checks it: both players are here and not bots, the item is real and not a
//!    starter, and the amount is sane. It hands the offer to the receiver.
//! 3. The receiver sees the same screen and accepts or declines.
//! 4. The host says it's done (or not, and why). The giver's bank lets the item go, or takes it
//!    back. The receiver's bank takes it in. Both record it in their history.
//!
//! Only the owner and admins can gift something they don't have ("made new"). Such a gift
//! counts on the receiver's computer only with a proof sent straight from the admin over Steam
//! (crate::commands::Direct::GiftSeal), checked against the receiver's own owners list.

use crate::game::*;
use crate::hub::HubEv;
use crate::items::{ItemKey, ItemStore};
use crate::roles::Role;

/// A gift in flight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Offer {
    pub gid: u32,
    pub from: u8,
    pub to: u8,
    pub item: ItemKey,
    pub n: u32,
    /// Made new by an admin (nothing leaves the giver's bank).
    pub create: bool,
}

/// How long an offer waits for its answer.
pub const ANSWER_TIME: f32 = 60.0;

#[derive(Clone, Debug, Default)]
pub struct Gifts {
    /// Ours, waiting for an answer.
    pub outgoing: Option<Offer>,
    /// Offered to us: the confirm screen.
    pub incoming: Option<Offer>,
    /// (host) offers in flight and their time left.
    pub pending: Vec<(Offer, f32)>,
    /// Proofs from admins (gift id, their Steam ID), for gifts made new.
    pub seals: Vec<(u32, u64)>,
    counter: u32,
}

/// Why the host turns an offer down (None: it's fine).
pub fn check(g: &Game, from: usize, o: &Offer) -> Option<&'static str> {
    let to = o.to as usize;
    if o.from as usize != from {
        return Some("that isn't your gift");
    }
    if to >= g.f.len() || !g.f[to].present {
        return Some("they're not here");
    }
    if to == from {
        return Some("you can't gift yourself");
    }
    if g.f[to].is_bot || g.f[from].is_bot {
        return Some("regulars (bots) have no bank");
    }
    if !o.item.valid() || o.item.starter() {
        return Some("that item can't be gifted");
    }
    if o.n == 0 || o.n > 99 {
        return Some("that many can't be gifted");
    }
    if o.create && g.role_of(from) < Role::Admin {
        return Some("only the owner and admins can make items new");
    }
    if g.hub.as_ref().is_some_and(|h| h.pending_from(from)) {
        return Some("one gift at a time");
    }
    None
}

impl crate::hub::Hub {
    fn pending_from(&self, from: usize) -> bool {
        self.gifts.pending.iter().any(|(o, _)| o.from as usize == from)
    }
}

/// Send a gift (after the giver confirmed it). Err says why it can't go.
pub fn offer(g: &mut Game, to: usize, item: ItemKey, n: u32, create: bool) -> Result<u32, String> {
    let me = g.local;
    if create && g.role_of(me) < Role::Admin {
        return Err("Only the owner and admins can make items new.".into());
    }
    let Some(h) = g.hub.as_mut() else { return Err("Gifts work in Varrow Centre.".into()) };
    if h.gifts.outgoing.is_some() {
        return Err("Your last gift is still waiting for an answer.".into());
    }
    h.gifts.counter = h.gifts.counter.wrapping_add(1);
    let gid = ((me as u32) << 24) | ((g.time * 10.0) as u32 & 0xffff) << 8 | (h.gifts.counter & 0xff);
    if !create {
        h.bank.hold_for_gift(gid, item, n).map_err(|e| match e {
            crate::items::StoreError::Starter => "Everyone has that one: it can't be gifted.".to_string(),
            crate::items::StoreError::NotOwned => "You don't have that many.".to_string(),
        })?;
        h.bank.apply_to(&mut h.earned);
    }
    let o = Offer { gid, from: me as u8, to: to as u8, item, n, create };
    h.gifts.outgoing = Some(o);
    if create {
        let id = g.admin.ids.get(to).copied().unwrap_or(0);
        if id != 0 {
            g.admin.direct_out.push((id, crate::commands::Direct::GiftSeal { gid }.to_bytes()));
        }
    }
    g.hub_send(HubEv::GiftOffer { gid, from: me as u8, to: to as u8, item: item.to_bytes(), n: n as u16, create });
    Ok(gid)
}

/// Accept (or decline) the gift offered to us.
pub fn answer(g: &mut Game, accept: bool) {
    let me = g.local;
    let Some(o) = g.hub.as_ref().and_then(|h| h.gifts.incoming) else { return };
    g.hub_send(HubEv::GiftAnswer { gid: o.gid, by: me as u8, accept });
}

/// The gift events (`from`: who sent it, as the host sees it; the host, as a player sees it).
pub fn on_hub(g: &mut Game, from: usize, e: HubEv) {
    let auth = g.is_authority();
    let me = g.local;
    match e {
        HubEv::GiftOffer { gid, from: f, to, item, n, create } => {
            let Some(item) = ItemKey::from_bytes(item) else { return };
            let o = Offer { gid, from: f, to, item, n: n as u32, create };
            if auth {
                if let Some(why) = check(g, from, &o) {
                    done(g, o, false, why);
                    return;
                }
                if let Some(h) = g.hub.as_mut() {
                    h.gifts.pending.push((o, ANSWER_TIME));
                }
                if to as usize == me {
                    if let Some(h) = g.hub.as_mut() {
                        h.gifts.incoming = Some(o);
                    }
                } else if g.kind == Kind::Host {
                    g.out_events.push(Ev::Hub(HubEv::GiftOffer { gid, from: f, to, item: item.to_bytes(), n, create }));
                }
            } else if to as usize == me {
                if let Some(h) = g.hub.as_mut() {
                    h.gifts.incoming = Some(o);
                }
            }
        }
        HubEv::GiftAnswer { gid, accept, .. } if auth => {
            let Some(k) = g.hub.as_ref().and_then(|h| h.gifts.pending.iter().position(|(o, _)| o.gid == gid && o.to as usize == from)) else { return };
            let o = g.hub.as_mut().unwrap().gifts.pending.remove(k).0;
            done(g, o, accept, if accept { "" } else { "they said no thanks" });
        }
        HubEv::GiftDone { gid, from: f, to, item, n, ok, why } if !auth || from == me => {
            if let Some(item) = ItemKey::from_bytes(item) {
                settle(g, Offer { gid, from: f, to, item, n: n as u32, create: false }, ok, &why);
            }
        }
        _ => {}
    }
}

/// (host) The gift is settled: everyone hears (and the host acts on it too).
fn done(g: &mut Game, o: Offer, ok: bool, why: &str) {
    let ev = HubEv::GiftDone { gid: o.gid, from: o.from, to: o.to, item: o.item.to_bytes(), n: o.n as u16, ok, why: why.to_string() };
    if g.kind == Kind::Host {
        g.out_events.push(Ev::Hub(ev));
    }
    settle(g, o, ok, why);
}

/// Our side of a settled gift.
fn settle(g: &mut Game, o: Offer, ok: bool, why: &str) {
    let me = g.local;
    let name = |s: u8| g.f.get(s as usize).map(|f| f.name.clone()).unwrap_or_else(|| "someone".into());
    let (giver, taker) = (name(o.from), name(o.to));
    let giver_id = g.admin.ids.get(o.from as usize).copied().unwrap_or(0);
    let Some(h) = g.hub.as_mut() else { return };
    if o.from as usize == me {
        let mine = h.gifts.outgoing.filter(|x| x.gid == o.gid);
        if let Some(x) = mine {
            if !x.create {
                h.bank.settle_gift(o.gid, ok, &taker);
                h.bank.apply_to(&mut h.earned);
            }
            h.gifts.outgoing = None;
            h.toast(if ok { format!("{taker} took your gift: {} {}", x.n, x.item.name()) } else { format!("Your gift to {taker} didn't go: {why}") });
        }
    }
    if o.to as usize == me {
        let mine = h.gifts.incoming.filter(|x| x.gid == o.gid);
        h.gifts.incoming = None;
        if ok {
            let Some(x) = mine else { return };
            if x.create {
                // made new: only with an admin's own proof, checked against our list
                // (a proof is only kept when it came from an admin on our list: commands::on_direct)
                let sealed = h.gifts.seals.iter().any(|&(gid, id)| gid == o.gid && (giver_id == 0 || id == giver_id));
                if !sealed {
                    h.toast(format!("{giver}'s gift wasn't proven to come from an admin: it's ignored"));
                    return;
                }
            }
            h.bank.grant(x.item, x.n, &format!("gift from {giver}"));
            h.bank.apply_to(&mut h.earned);
            h.toast(format!("{giver} gave you {} {}", x.n, x.item.name()));
        }
    }
}

/// A made-new gift's proof only counts from an admin on our own list (checked on arrival in
/// crate::commands::on_direct, which drops proofs from anyone else).
pub fn tick(g: &mut Game, dt: f32) {
    if !g.is_authority() {
        return;
    }
    let mut expired = Vec::new();
    if let Some(h) = g.hub.as_mut() {
        for (o, t) in h.gifts.pending.iter_mut() {
            *t -= dt;
            if *t <= 0.0 {
                expired.push(*o);
            }
        }
        h.gifts.pending.retain(|(_, t)| *t > 0.0);
    }
    for o in expired {
        done(g, o, false, "no answer");
    }
    // a giver who left: their offers end
    let gone: Vec<Offer> = g.hub.as_ref().map_or(Vec::new(), |h| h.gifts.pending.iter().map(|p| p.0).filter(|o| !g.f.get(o.from as usize).is_some_and(|f| f.present) || !g.f.get(o.to as usize).is_some_and(|f| f.present)).collect());
    for o in gone {
        if let Some(h) = g.hub.as_mut() {
            h.gifts.pending.retain(|p| p.0.gid != o.gid);
        }
        done(g, o, false, "a player left");
    }
}
