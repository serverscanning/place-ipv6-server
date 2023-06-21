//! Track PPS per user.
//! A lot of fail safes are built-in to prevent abuse by people with
//! a lot of IPs, spoofing random ones or other kinds of silliness.

use fxhash::FxHashMap;
use once_cell::sync::Lazy;
use std::{
    net::Ipv6Addr,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PpsPublicUser {
    pub id: u64,
}
pub static PPS_USERS: Lazy<Mutex<FxHashMap<PpsPrivateUser48, PpsUserInfo>>> =
    Lazy::new(|| Default::default());
pub static PPS_NEXT_USER_ID: Mutex<PpsPublicUser> = Mutex::new(PpsPublicUser { id: 1 });
pub static PPS_USERS_DISABLED_UNTIIL: Mutex<Option<Instant>> = Mutex::new(None);
#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct PpsPrivateUser48 {
    /// First 3 ipv6 segements
    prefix: [u16; 3],
}

impl PpsPrivateUser48 {
    pub fn from_addr(user_address: Ipv6Addr) -> Self {
        let s = user_address.segments();
        Self {
            prefix: [s[0], s[1], s[2]],
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct PpsPrivateUser64 {
    /// First 4 ipv6 segements
    prefix: [u16; 4],
}

impl PpsPrivateUser64 {
    pub fn from_addr(user_address: Ipv6Addr) -> Self {
        let s = user_address.segments();
        Self {
            prefix: [s[0], s[1], s[2], s[3]],
        }
    }
}

pub enum PpsUserInfo {
    User64 {
        data_map: FxHashMap<PpsPrivateUser64, PpsUserInfoData>,
    },
    User48 {
        data: PpsUserInfoData,
    },
}

pub struct PpsUserInfoData {
    last_seen: Instant,
    user_id: PpsPublicUser,
    pub pps_counter: usize,
}

impl PpsUserInfoData {
    pub fn get_user_id(&self) -> PpsPublicUser {
        self.user_id
    }
}

pub fn is_disabled(
    pps_users: &mut FxHashMap<PpsPrivateUser48, PpsUserInfo>,
    pps_users_disabled_until: &mut Option<Instant>,
    now: Instant,
) -> bool {
    // Temporarily disabled because someone keeps fucking with a lot of IPs or spoofing them!
    if let Some(disabled_until) = pps_users_disabled_until {
        if *disabled_until > now {
            true
        } else {
            *pps_users_disabled_until = None;
            false
        }
    } else {
        if pps_users.len() >= 1024 {
            // Someone is fucking with a lot of IPs or spoofs them. Disabling for 30 seconds!
            pps_users.clear();
            *pps_users_disabled_until = Some(now + Duration::from_secs(30));
            true
        } else {
            false
        }
    }
}

pub fn ensure_existing_activity_updated_and_migrated(
    pps_users: &mut FxHashMap<PpsPrivateUser48, PpsUserInfo>,
    next_user_id: &mut PpsPublicUser,
    now: Instant,
    user_ip: Ipv6Addr,
) {
    if pps_users.len() >= 1024 {
        // Someone is fucking with a lot of IPs or spoofs them. Do not add any further entries!
        return;
    }

    let user_48 = PpsPrivateUser48::from_addr(user_ip);

    let user_info = pps_users.entry(user_48).or_insert_with(|| {
        let new_user_id = *next_user_id;
        *next_user_id = PpsPublicUser {
            id: new_user_id.id + 1,
        };
        let user_64 = PpsPrivateUser64::from_addr(user_ip);

        let mut data_map = FxHashMap::default();
        data_map.insert(
            user_64,
            PpsUserInfoData {
                last_seen: now,
                user_id: new_user_id,
                pps_counter: 0,
            },
        );
        PpsUserInfo::User64 { data_map }
    });

    let migrate_to_user48_info = match user_info {
        PpsUserInfo::User48 { data } => {
            data.last_seen = now;
            None
        }
        PpsUserInfo::User64 { data_map } => {
            if data_map.len() < 1024 {
                let user_64 = PpsPrivateUser64::from_addr(user_ip);
                let data = data_map.entry(user_64).or_insert_with(|| {
                    let new_user_id = *next_user_id;
                    *next_user_id = PpsPublicUser {
                        id: new_user_id.id + 1,
                    };
                    PpsUserInfoData {
                        last_seen: now,
                        user_id: new_user_id,
                        pps_counter: 0,
                    }
                });
                data.last_seen = now;
                None
            } else {
                Some(data_map.values().min_by_key(|data| data.user_id.id).expect(
                    "PpsUserInfo::User64.data_map should never be empty because of earlier check!",
                ).to_owned())
            }
        }
    };

    if let Some(user48_data) = migrate_to_user48_info {
        // Migrate User64 to User48
        let user48_migration_info = PpsUserInfoData {
            last_seen: now,
            user_id: user48_data.user_id.clone(),
            pps_counter: 0,
        };

        // Not returned from above means the user was User64 and had to many entries. Migrate from User64 to User48 first
        let new_user_info = PpsUserInfo::User48 {
            data: user48_migration_info,
        };
        pps_users.insert(user_48, new_user_info);
    }
}

pub fn cleanup(pps_users: &mut FxHashMap<PpsPrivateUser48, PpsUserInfo>, now: Instant) {
    pps_users.retain(|_, user_info| match user_info {
        PpsUserInfo::User48 { data } => now - data.last_seen < Duration::from_secs(60 * 60),
        PpsUserInfo::User64 { data_map } => {
            data_map.retain(|_, data| now - data.last_seen < Duration::from_secs(60 * 60));
            data_map.len() > 0
        }
    });
}

pub fn get_all_pps_counters_and_reset(
    pps_users: &mut FxHashMap<PpsPrivateUser48, PpsUserInfo>,
) -> FxHashMap<PpsPublicUser, usize> {
    let mut map = FxHashMap::default();
    for user_info in pps_users.values_mut() {
        match user_info {
            PpsUserInfo::User48 { data } => {
                map.insert(data.user_id, data.pps_counter);
                data.pps_counter = 0;
            }
            PpsUserInfo::User64 { data_map } => {
                for data in data_map.values_mut() {
                    map.insert(data.user_id, data.pps_counter);
                    data.pps_counter = 0;
                }
            }
        }
    }
    map
}

pub fn find_user_info_data<'a>(
    pps_users: &'a FxHashMap<PpsPrivateUser48, PpsUserInfo>,
    user_ip: Ipv6Addr,
) -> Option<&'a PpsUserInfoData> {
    pps_users
        .get(&PpsPrivateUser48::from_addr(user_ip))
        .and_then(|info| match info {
            PpsUserInfo::User48 { data } => Some(data),
            PpsUserInfo::User64 { data_map } => data_map.get(&PpsPrivateUser64::from_addr(user_ip)),
        })
}

pub fn find_user_info_data_mut<'a>(
    pps_users: &'a mut FxHashMap<PpsPrivateUser48, PpsUserInfo>,
    user_ip: Ipv6Addr,
) -> Option<&'a mut PpsUserInfoData> {
    pps_users
        .get_mut(&PpsPrivateUser48::from_addr(user_ip))
        .and_then(|info| match info {
            PpsUserInfo::User48 { data } => Some(data),
            PpsUserInfo::User64 { data_map } => {
                data_map.get_mut(&PpsPrivateUser64::from_addr(user_ip))
            }
        })
}
