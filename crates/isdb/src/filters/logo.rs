//! ロゴを取得するフィルター。

use fxhash::{FxHashMap, FxHashSet};

use crate::data_module;
use crate::demux;
use crate::desc::{self, ServiceType, StreamType};
use crate::dsmcc;
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
    es_pids: FxHashSet<Pid>,

    services: FxHashMap<u16, Service>,
    versions: FxHashMap<u32, u16>,
    logo_downloads: FxHashMap<u16, dsmcc::download::DownloadData>,

    callback: F,
}

struct Service {
    pmt_pid: Pid,
    service_type: ServiceType,
    streams: Vec<Pid>,
}

mod sealed {
    // モジュール直下に定義するとE0446で怒られるので封印
    #[derive(Debug, Clone, Copy)]
    pub enum Tag {
        Pat,
        Nit,
        Cdt,
        Pmt,
        Es,
    }
}

type Tag = sealed::Tag;

impl<F: FnMut(&LogoData)> LogoDownloadFilter<F> {
    /// `LogoDownloadFilter`を生成する。
    ///
    /// ロゴを取得する度`f`が呼ばれる。
    pub fn new(f: F) -> LogoDownloadFilter<F> {
        LogoDownloadFilter {
            es_pids: FxHashSet::default(),

            services: FxHashMap::default(),
            versions: FxHashMap::default(),
            logo_downloads: FxHashMap::default(),

            callback: f,
        }
    }

    fn on_pat(&mut self, ctx: &mut demux::Context<Tag>, psi: &PsiSection) {
        let Some(pat) = table::Pat::read(psi) else {
            return;
        };

        for (_, service) in self.services.drain() {
            ctx.table().unset(service.pmt_pid);
        }

        for program in &*pat.pmts {
            self.services.insert(
                program.program_number.get(),
                Service {
                    pmt_pid: program.program_map_pid,
                    service_type: ServiceType::INVALID,
                    streams: Vec::new(),
                },
            );
            ctx.table().set_as_psi(program.program_map_pid, Tag::Pmt);
        }
    }

    fn on_nit(&mut self, ctx: &mut demux::Context<Tag>, psi: &PsiSection) {
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
                            ctx.table().set_as_psi(pid, Tag::Es);
                        }
                    } else if service.service_type == ServiceType::ENGINEERING {
                        for &pid in &*service.streams {
                            self.es_pids.remove(&pid);
                            ctx.table().unset(pid);
                        }
                    }

                    service.service_type = new_svc.service_type;
                }
            }
        }
    }

    fn on_cdt(&mut self, _: &mut demux::Context<Tag>, psi: &PsiSection) {
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

    fn on_pmt(&mut self, ctx: &mut demux::Context<Tag>, psi: &PsiSection) {
        let Some(pmt) = table::Pmt::read(psi) else {
            return;
        };

        let Some(service) = self.services.get_mut(&pmt.program_number) else {
            log::debug!("PMT: unknown program number");
            return;
        };
        if service.service_type == ServiceType::ENGINEERING {
            for &pid in &*service.streams {
                self.es_pids.remove(&pid);
                ctx.table().unset(pid);
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
                ctx.table().set_as_psi(stream.elementary_pid, Tag::Es);
            }
        }
    }

    fn on_es(&mut self, _: &mut demux::Context<Tag>, psi: &PsiSection) {
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
    type Tag = Tag;

    fn on_pes_packet(&mut self, _: &mut demux::Context<Tag>, _: &PesPacket) {}

    fn on_setup(&mut self) -> demux::Table<Tag> {
        let mut table = demux::Table::new();
        table.set_as_psi(Pid::PAT, Tag::Pat);
        table.set_as_psi(Pid::NIT, Tag::Nit);
        table.set_as_psi(Pid::CDT, Tag::Cdt);
        table
    }

    fn on_psi_section(&mut self, ctx: &mut demux::Context<Tag>, psi: &PsiSection) {
        match ctx.tag() {
            Tag::Pat => self.on_pat(ctx, psi),
            Tag::Nit => self.on_nit(ctx, psi),
            Tag::Cdt => self.on_cdt(ctx, psi),
            Tag::Pmt => self.on_pmt(ctx, psi),
            Tag::Es => self.on_es(ctx, psi),
        }
    }
}
