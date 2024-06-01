use image_dds::ddsfile::{
    AlphaMode, Caps2, D3D10ResourceDimension, D3DFormat, Dds, DxgiFormat, FourCC, NewDxgiParams,
};
use thiserror::Error;

use crate::{Bntx, SurfaceFormat};

#[derive(Debug, Error)]
pub enum CreateBntxError {
    #[error("failed to swizzle surface")]
    SwizzleError(#[from] tegra_swizzle::SwizzleError),
    #[error("the given DDS format is not supported")]
    UnsupportedImageFormat,
}

// TODO: use image_dds
// TODO: make this a method?
// TODO: Create an error type for this.
pub fn create_dds(bntx: &Bntx) -> Result<Dds, tegra_swizzle::SwizzleError> {
    Ok(image_dds::Surface {
        height: bntx.height(),
        width: bntx.width(),
        depth: bntx.depth(),
        layers: bntx.layer_count(),
        mipmaps: bntx.mipmap_count(),
        image_format: bntx.image_format().into(),
        data: bntx.deswizzled_data()?,
    }
    .to_dds()
    .unwrap())
}

impl From<SurfaceFormat> for image_dds::ImageFormat {
    fn from(value: SurfaceFormat) -> Self {
        match value {
            SurfaceFormat::R8Unorm => Self::R8Unorm,
            SurfaceFormat::Unk1 => todo!(),
            SurfaceFormat::R8G8B8A8Unorm => Self::Rgba8Unorm,
            SurfaceFormat::R8G8B8A8Srgb => Self::Rgba8UnormSrgb,
            SurfaceFormat::B8G8R8A8Unorm => Self::Bgra8Unorm,
            SurfaceFormat::B8G8R8A8Srgb => Self::Bgra8UnormSrgb,
            SurfaceFormat::R11G11B10 => todo!(), // TODO: Add support to image_dds
            SurfaceFormat::BC1Unorm => Self::BC1RgbaUnorm,
            SurfaceFormat::BC1Srgb => Self::BC1RgbaUnormSrgb,
            SurfaceFormat::BC2Unorm => Self::BC2RgbaUnorm,
            SurfaceFormat::BC2Srgb => Self::BC2RgbaUnormSrgb,
            SurfaceFormat::BC3Unorm => Self::BC3RgbaUnorm,
            SurfaceFormat::BC3Srgb => Self::BC3RgbaUnormSrgb,
            SurfaceFormat::BC4Unorm => Self::BC4RUnorm,
            SurfaceFormat::BC4Snorm => Self::BC4RSnorm,
            SurfaceFormat::BC5Unorm => Self::BC5RgUnorm,
            SurfaceFormat::BC5Snorm => Self::BC5RgSnorm,
            SurfaceFormat::BC6Sfloat => Self::BC6hRgbSfloat,
            SurfaceFormat::BC6Ufloat => Self::BC6hRgbUfloat,
            SurfaceFormat::BC7Unorm => Self::BC7RgbaUnorm,
            SurfaceFormat::BC7Srgb => Self::BC7RgbaUnormSrgb,
        }
    }
}
