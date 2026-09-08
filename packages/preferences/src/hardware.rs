// CreativeSuiteOpen — Suite créative professionnelle open source
// Copyright (C) 2026 vabyz971
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Détection matérielle : CPU, RAM (best-effort) et adaptateurs GPU réels
//! énumérés via wgpu — les mêmes backends que ceux utilisés au rendu.

use serde::{Deserialize, Serialize};

/// Un GPU détecté.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub driver: String,
    /// Backend wgpu (« Vulkan », « Metal », « Dx12 »…)
    pub api: String,
    pub is_discrete: bool,
}

/// Informations CPU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub name: String,
    pub cores: usize,
}

/// Informations mémoire (best-effort ; 0 = inconnu).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RamInfo {
    pub total_mb: u64,
}

/// Rapport matériel complet.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HardwareReport {
    pub cpu: Option<CpuInfo>,
    pub ram: RamInfo,
    pub gpus: Vec<GpuInfo>,
}

impl HardwareReport {
    /// Détection synchrone — à appeler depuis une tâche de fond
    /// (`Task::perform`) pour ne jamais bloquer l'interface.
    #[must_use]
    pub fn detect_sync() -> Self {
        let cpu = Some(CpuInfo {
            name: std::env::consts::ARCH.to_string(),
            cores: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
        });

        let ram = RamInfo {
            total_mb: detect_ram_mb(),
        };

        let mut gpus = Vec::new();
        // Même pattern d'instance que photo-engine/src/gpu.rs (headless-safe)
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::all();
        let instance = wgpu::Instance::new(desc);

        // enumerate_adapters est async en wgpu 30 : blocage ponctuel via
        // pollster, appel prévu depuis une tâche de fond (jamais l'UI).
        let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
        for adapter in adapters {
            let info = adapter.get_info();
            gpus.push(GpuInfo {
                name: if info.name.is_empty() {
                    format!("GPU {:?}", info.backend)
                } else {
                    info.name.clone()
                },
                vendor: format!("{:X}", info.vendor),
                driver: info.driver,
                api: format!("{:?}", info.backend),
                is_discrete: info.device_type == wgpu::DeviceType::DiscreteGpu,
            });
        }
        gpus.sort_by(|a, b| b.is_discrete.cmp(&a.is_discrete).then(a.name.cmp(&b.name)));

        Self { cpu, ram, gpus }
    }

    /// Variante async (convention mission) — le corps est synchrone mais
    /// l'appel s'intègre dans n'importe quel runtime.
    pub async fn detect() -> Self {
        Self::detect_sync()
    }
}

/// RAM totale en Mo — lecture best-effort de /proc/meminfo sous Linux,
/// `sysinfo` non embarqué volontairement (dépendance lourde pour une
/// ligne d'information). Retourne 0 si indéterminé.
fn detect_ram_mb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo")
            && let Some(line) = content.lines().find(|l| l.starts_with("MemTotal:"))
            && let Some(kb) = line.split_whitespace().nth(1)
            && let Ok(kb) = kb.parse::<u64>()
        {
            return kb / 1024;
        }
        0
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_ne_panique_jamais_et_renvoie_le_cpu() {
        // Sur machine CI headless : zéro GPU possible, CPU toujours présent,
        // et surtout AUCUN panic — contrat du rapport matériel.
        let report = HardwareReport::detect_sync();
        let cpu = report.cpu.expect("cpu info");
        assert!(cpu.cores >= 1);
        assert!(!cpu.name.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ram_linux_est_lue_de_proc_meminfo() {
        let mb = detect_ram_mb();
        assert!(mb > 0, "/proc/meminfo doit fournir MemTotal");
    }
}
