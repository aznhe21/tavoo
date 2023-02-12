//! ロゴを取得するフィルター。

use fxhash::{FxHashMap, FxHashSet};

use crate::data_module;
use crate::demux;
use crate::desc::{self, ServiceType, StreamType};
use crate::dsmcc;
use crate::packet::Packet;
use crate::pes::PesPacket;
use crate::pid::Pid;
use crate::psi::PsiSection;
use crate::table;

/// 取得したロゴ。
#[derive(Debug)]
pub struct LogoData<'a> {
    /// 所属するネットワークID。
    pub network_id: u16,

    /// 種別。
    pub logo_type: data_module::LogoType,

    /// ID。
    pub logo_id: u16,

    /// バージョン。
    pub logo_version: u16,

    /// 受信したPNGデータ。
    ///
    /// このデータはARIB STD-B21で規定される一部チャンクが省略されたPNG形式であり、
    /// そのまま保存しても通常のPNGファイルとして使用することは出来ない。
    pub data: &'a [u8],
}

/// ロゴを取得するためのフィルター。
pub struct LogoDownloadFilter<F> {
    pmt_pids: FxHashSet<Pid>,
    es_pids: FxHashSet<Pid>,

    services: FxHashMap<u16, Service>,
    versions: FxHashMap<u32, u16>,
    logo_downloads: FxHashMap<u16, dsmcc::download::DownloadData>,

    callback: F,
}

struct Service {
    service_type: ServiceType,
    streams: Vec<Pid>,
}

impl<F: FnMut(&LogoData)> LogoDownloadFilter<F> {
    /// `LogoDownloadFilter`を生成する。
    ///
    /// ロゴを取得する度`f`が呼ばれる。
    pub fn new(f: F) -> LogoDownloadFilter<F> {
        LogoDownloadFilter {
            pmt_pids: FxHashSet::default(),
            es_pids: FxHashSet::default(),

            services: FxHashMap::default(),
            versions: FxHashMap::default(),
            logo_downloads: FxHashMap::default(),

            callback: f,
        }
    }

    fn on_pat(&mut self, psi: &PsiSection) {
        let Some(pat) = table::Pat::read(psi) else {
            return;
        };

        self.pmt_pids.clear();
        self.services.clear();
        for program in &*pat.pmts {
            self.pmt_pids.insert(program.program_map_pid);
            self.services.insert(
                program.program_number.get(),
                Service {
                    service_type: ServiceType::INVALID,
                    streams: Vec::new(),
                },
            );
        }
    }

    fn on_nit(&mut self, psi: &PsiSection) {
        let Some(nit) = table::Nit::read(psi) else {
            return;
        };

        for ts in &*nit.transport_streams {
            let Some(sld) = ts.transport_descriptors.get::<desc::ServiceListDescriptor>() else {
                continue;
            };

            for new_svc in &*sld.services {
                let Some(service) = self.services.get_mut(&new_svc.service_id) else {
                    continue;
                };

                if service.service_type != new_svc.service_type {
                    if new_svc.service_type == ServiceType::ENGINEERING {
                        for &pid in &*service.streams {
                            self.es_pids.insert(pid);
                        }
                    } else if service.service_type == ServiceType::ENGINEERING {
                        for pid in &*service.streams {
                            self.es_pids.remove(pid);
                        }
                    }

                    service.service_type = new_svc.service_type;
                }
            }
        }
    }

    fn on_cdt(&mut self, psi: &PsiSection) {
        let Some(cdt) = table::Cdt::read(psi) else {
            return;
        };
        if cdt.data_type != table::CdtDataType::LOGO {
            return;
        }

        let network_id = cdt.original_network_id;
        let Some(logo) = data_module::CdtLogo::read(cdt.data_module) else {
            return;
        };

        if logo.logo_type.is_known() && !logo.data.is_empty() {
            let logo = LogoData {
                network_id,
                logo_type: logo.logo_type,
                logo_id: logo.logo_id,
                logo_version: logo.logo_version,
                data: logo.data,
            };
            (self.callback)(&logo);
        }
    }

    fn on_pmt(&mut self, _pid: Pid, psi: &PsiSection) {
        let Some(pmt) = table::Pmt::read(psi) else {
            return;
        };

        let Some(service) = self.services.get_mut(&pmt.program_number) else {
            log::debug!("PMT: unknown program number");
            return;
        };
        if service.service_type == ServiceType::ENGINEERING {
            for pid in &*service.streams {
                self.es_pids.remove(pid);
            }
        }

        service.streams.clear();
        for stream in &*pmt.streams {
            if stream.stream_type != StreamType::DATA_CARROUSEL {
                continue;
            }

            let Some(stream_id) = stream.descriptors.get::<desc::StreamIdDescriptor>() else {
                continue;
            };
            if !matches!(stream_id.component_tag, 0x79 | 0x7A) {
                continue;
            }

            service.streams.push(stream.elementary_pid);
            if service.service_type == ServiceType::ENGINEERING {
                self.es_pids.insert(stream.elementary_pid);
            }
        }
    }

    fn on_es(&mut self, _pid: Pid, psi: &PsiSection) {
        match dsmcc::table::DsmccSection::read(psi) {
            Some(dsmcc::table::DsmccSection::Dii(dii)) => {
                for module in &*dii.modules {
                    let Some(name) = module.module_info.get::<dsmcc::desc::NameDescriptor>() else {
                        continue;
                    };
                    let text = name.text.as_bytes();
                    if !text.starts_with(b"LOGO-0") && !text.starts_with(b"CS_LOGO-0") {
                        continue;
                    }

                    // log::trace!("DII Logo Data [PID {:04x}] : Download ID {:08x} / Module ID {:04x} / Module size {}",
                    //             pid, dii.download_id, module.module_id, module.module_size);

                    match self.logo_downloads.entry(module.module_id) {
                        std::collections::hash_map::Entry::Occupied(mut entry) => {
                            if entry.get().needs_restart(&dii, module) {
                                entry.insert(dsmcc::download::DownloadData::new(&dii, module));
                            }
                        }
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            entry.insert(dsmcc::download::DownloadData::new(&dii, module));
                        }
                    }
                }
            }
            Some(dsmcc::table::DsmccSection::Ddb(ddb)) => {
                let Some(dd) = self.logo_downloads.get_mut(&ddb.module_id) else {
                    return;
                };
                let Some(data) = dd.store(&ddb) else {
                    // ダウンロード進行中
                    return;
                };

                let Some(logo) = data_module::Logo::read(data) else {
                    return;
                };
                if !logo.logo_type.is_known() {
                    return;
                }

                // if log::log_enabled!(log::Level::Trace) {
                //     for (i, info) in logo.logos.iter().enumerate() {
                //         log::trace!(
                //             "[{}/{}] Logo ID {:04X} / {} Services",
                //             i + 1,
                //             logo.logos.len(),
                //             info.logo_id,
                //             info.services.len(),
                //         );
                //         for (j, service) in info.services.iter().enumerate() {
                //             log::trace!(
                //                 "[{}:{:02}/{:02}] Network ID {:04X} / TSID {:04X} / Service ID {:04X}",
                //                 i + 1,
                //                 j + 1,
                //                 info.services.len(),
                //                 service.original_network_id,
                //                 service.transport_stream_id,
                //                 service.service_id,
                //             );
                //         }
                //     }
                // }

                let logo_version = self
                    .versions
                    .get(&ddb.header.download_id)
                    .copied()
                    .unwrap_or(0);
                for info in &*logo.logos {
                    if info.services.is_empty() || info.data.is_empty() {
                        continue;
                    }

                    let logo = LogoData {
                        network_id: info.services[0].original_network_id,
                        logo_type: logo.logo_type,
                        logo_id: info.logo_id,
                        logo_version,
                        data: info.data,
                    };
                    (self.callback)(&logo);
                }
            }
            _ => {}
        }
    }
}

impl<F: FnMut(&LogoData)> demux::Filter for LogoDownloadFilter<F> {
    fn on_pes_packet(&mut self, _: Pid, _: &PesPacket) {}

    fn on_packet(&mut self, packet: &Packet) -> Option<demux::PacketType> {
        let pid = packet.pid();

        let is_psi = matches!(pid, Pid::PAT | Pid::NIT | Pid::CDT)
            || self.pmt_pids.contains(&pid)
            || self.es_pids.contains(&pid);
        is_psi.then_some(demux::PacketType::Psi)
    }

    fn on_psi_section(&mut self, pid: Pid, psi: &PsiSection) {
        match pid {
            Pid::PAT => self.on_pat(psi),
            Pid::NIT => self.on_nit(psi),
            Pid::CDT => self.on_cdt(psi),
            pid if self.pmt_pids.contains(&pid) => self.on_pmt(pid, psi),
            pid if self.es_pids.contains(&pid) => self.on_es(pid, psi),
            _ => {}
        }
    }
}
