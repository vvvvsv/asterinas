// SPDX-License-Identifier: MPL-2.0

use alloc::{boxed::Box, sync::Arc, vec::Vec};

use ostd::{
    bus::{
        pci::{
            bus::{PciDevice, PciDriver},
            capability::CapabilityData,
            common_device::PciCommonDevice,
        },
        BusProbeError,
    },
    sync::SpinLock,
};

use super::device::VirtioPciModernTransport;
use crate::transport::{
    pci::{device::VirtioPciDevice, legacy::VirtioPciLegacyTransport},
    VirtioTransport,
};

#[derive(Debug)]
pub struct VirtioPciDriver {
    devices: SpinLock<Vec<Box<dyn VirtioTransport>>>,
}

impl VirtioPciDriver {
    pub fn pop_device_transport(&self) -> Option<Box<dyn VirtioTransport>> {
        self.devices.lock().pop()
    }

    pub(super) fn new() -> Self {
        VirtioPciDriver {
            devices: SpinLock::new(Vec::new()),
        }
    }
}

impl PciDriver for VirtioPciDriver {
    fn probe(
        &self,
        device: PciCommonDevice,
    ) -> Result<Arc<dyn PciDevice>, (BusProbeError, PciCommonDevice)> {
        const VIRTIO_DEVICE_VENDOR_ID: u16 = 0x1af4;
        if device.device_id().vendor_id != VIRTIO_DEVICE_VENDOR_ID {
            return Err((BusProbeError::DeviceNotMatch, device));
        }

        let has_vendor_cap = device
            .capabilities()
            .iter()
            .any(|cap| matches!(cap.capability_data(), CapabilityData::Vndr(_)));
        let device_id = *device.device_id();
        let transport: Box<dyn VirtioTransport> = match device_id.device_id {
            0x1000..0x1040 if (device.device_id().revision_id == 0) => {
                if has_vendor_cap {
                    let modern = VirtioPciModernTransport::new(device)?;
                    Box::new(modern)
                } else {
                    let legacy = VirtioPciLegacyTransport::new(device)?;
                    Box::new(legacy)
                }
            }
            0x1040..0x107f => {
                if !has_vendor_cap {
                    return Err((BusProbeError::DeviceNotMatch, device));
                }
                let modern = VirtioPciModernTransport::new(device)?;
                Box::new(modern)
            }
            _ => return Err((BusProbeError::DeviceNotMatch, device)),
        };
        self.devices.lock().push(transport);

        Ok(Arc::new(VirtioPciDevice::new(device_id)))
    }
}
