use std::{collections::{HashMap, BTreeMap}, path::PathBuf, fs, ops::Range};

use log::error;

use memmap2::Mmap;
use ptnet::image_header::{self, HWVersion};

pub struct Firmware {
    mmap: Mmap,
    pub header: image_header::Header,
    payload_range: Range<usize>
}

impl Firmware {
    pub fn payload(&self) -> &[u8] {
        &self.mmap[self.payload_range.clone()]
    }
}

pub type FirmwareMap = BTreeMap<image_header::FWVersion, Box<Firmware>>;

pub struct FirmwareIndex {
    map: HashMap<image_header::HWVersion, FirmwareMap>
}

impl FirmwareIndex {
    pub fn load_from(path: &PathBuf) -> Result<Self, std::io::Error> {
        let mut index = FirmwareIndex {
            map: HashMap::new()
        };

        for entry in fs::read_dir(path)? {
            let pth = entry?.path();
            match fs::File::open(&pth) {
                Ok(file) => {
                    let mmap_result = unsafe { Mmap::map(&file) };

                    if let Err(err) = mmap_result {
                        error!("Can't mmap firmware from '{}', skip! ({})", pth.to_str().unwrap_or_default(), err);
                        continue;
                    }

                    let mut fw = Box::new(Firmware {
                        mmap: mmap_result.unwrap(),
                        header: image_header::Header { raw: [0; 116] },
                        payload_range: 0..0
                    });

                    match image_header::Container::parse_from(&fw.mmap[..]) {
                        Ok((cont,pay_rng)) => {
                            let hw_version = &unsafe { cont.header.fields }.v0.hw_version;
                            let fw_version = &unsafe { cont.header.fields }.v0.fw_version;

                            fw.header = cont.header;
                            fw.payload_range = pay_rng;

                            match index.map.get_mut(hw_version) {
                                Some(fwmap) => {
                                    fwmap.insert(*fw_version, fw);
                                },
                                None => {
                                    let mut fwmap = BTreeMap::new();
                                    fwmap.insert(*fw_version, fw);
                                    index.map.insert(*hw_version, fwmap);
                                }
                            };
                        },
                        Err(err) => {
                            error!("Can't load firmware from '{}', skip! ({})", pth.to_str().unwrap_or_default(), err);
                        }
                    };
                },
                Err(err) => {
                    error!("Error loading file '{}', skip! ({})", pth.to_str().unwrap_or_default(), err);
                },
            }
        }

        Ok(index)
    }

    pub fn get_firmwares_for(&self, hw: &HWVersion) -> Option<&FirmwareMap> {
        self.map.get_key_value(hw).and_then(|x| Some(x.1))
    }
}