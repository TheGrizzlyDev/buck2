/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use sysinfo::Disks;

pub struct UnixSystemStats {
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
}

pub struct DiskSpaceStats {
    pub total_space: u64,
    pub available_space: u64,
}

impl UnixSystemStats {
    #[cfg(unix)]
    pub fn get() -> Option<Self> {
        let mut loadavg: [f64; 3] = [0.0, 0.0, 0.0];
        if unsafe { libc::getloadavg(&mut loadavg[0], 3) } != 3 {
            // This doesn't seem to set errno (or at least it's not documented to do so).
            return None;
        }
        Some(Self {
            load1: loadavg[0],
            load5: loadavg[1],
            load15: loadavg[2],
        })
    }

    #[cfg(not(unix))]
    pub fn get() -> Option<Self> {
        None
    }
}

pub fn disk_space_stats() -> Option<DiskSpaceStats> {
    let disks = Disks::new_with_refreshed_list();
    let root = if cfg!(windows) { "C:\\" } else { "/" };
    disks
        .iter()
        .find(|d| d.mount_point().as_os_str() == root)
        .map(|disk| DiskSpaceStats {
            total_space: disk.total_space(),
            available_space: disk.available_space(),
        })
}

pub fn system_memory_stats() -> u64 {
    use sysinfo::MemoryRefreshKind;
    use sysinfo::RefreshKind;
    use sysinfo::System;

    let system = System::new_with_specifics(
        RefreshKind::new().with_memory(MemoryRefreshKind::new().with_ram()),
    );
    system.total_memory()
}

#[cfg(test)]
mod tests {
    use super::disk_space_stats;
    use super::system_memory_stats;
    use super::DiskSpaceStats;

    #[test]
    fn get_system_memory_stats() {
        let total_mem = system_memory_stats();
        // sysinfo returns zero when fails to retrieve data
        assert!(total_mem > 0);
    }

    #[test]
    fn get_disk_space_stats() {
        if let Some(DiskSpaceStats {
            total_space,
            available_space,
        }) = disk_space_stats()
        {
            assert!(total_space > 0 && available_space > 0);
        };
    }
}
