use ddsfile::{
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
pub fn create_dds(bntx: &Bntx) -> Result<Dds, tegra_swizzle::SwizzleError> {
    let some_if_above_one = |x| if x > 0 { Some(x) } else { None };

    let mut dds = Dds::new_dxgi(NewDxgiParams {
        height: bntx.height(),
        width: bntx.width(),
        depth: some_if_above_one(bntx.depth()),
        format: bntx.image_format().into(),
        mipmap_levels: some_if_above_one(bntx.mipmap_count() as u32),
        array_layers: some_if_above_one(bntx.layer_count()),
        caps2: if bntx.depth() > 1 {
            Some(Caps2::VOLUME)
        } else {
            None
        },
        is_cubemap: bntx.layer_count() == 6,
        // TODO: Check the dimension instead?
        resource_dimension: if bntx.depth() > 1 {
            D3D10ResourceDimension::Texture3D
        } else {
            D3D10ResourceDimension::Texture2D
        },
        alpha_mode: AlphaMode::Unknown, // TODO: Alpha mode?
    })
    .unwrap();

    // DDS stores mipmaps in a contiguous region of memory.
    dds.data = bntx.deswizzled_data()?;

    Ok(dds)
}

// TODO: Make this a method?
// pub fn create_bntx(name: &str, dds: &Dds) -> Result<BntxFile, CreateBntxError> {
//     BntxFile::from_image_data(
//         name,
//         dds.get_width(),
//         dds.get_height(),
//         dds.get_depth(),
//         dds.get_num_mipmap_levels(),
//         layer_count(dds),
//         dds_image_format(dds).ok_or(CreateBntxError::UnsupportedImageFormat)?,
//         &dds.data,
//     )
//     .map_err(Into::into)
// }

fn layer_count(dds: &Dds) -> u32 {
    // Array layers for DDS are calculated differently for cube maps.
    if matches!(&dds.header10, Some(header10) if header10.misc_flag == ddsfile::MiscFlag::TEXTURECUBE)
    {
        dds.get_num_array_layers() * 6
    } else {
        dds.get_num_array_layers()
    }
}

fn dds_image_format(dds: &Dds) -> Option<SurfaceFormat> {
    // The format can be DXGI, D3D, or specified in the FOURCC.
    let dxgi = dds.get_dxgi_format();
    let d3d = dds.get_d3d_format();
    let fourcc = dds.header.spf.fourcc.as_ref();

    dxgi.and_then(image_format_from_dxgi)
        .or_else(|| d3d.and_then(image_format_from_d3d))
        .or_else(|| fourcc.and_then(image_format_from_fourcc))
}

fn image_format_from_dxgi(format: DxgiFormat) -> Option<SurfaceFormat> {
    match format {
        DxgiFormat::R8_UNorm => Some(SurfaceFormat::R8Unorm),
        DxgiFormat::R8G8B8A8_UNorm_sRGB => Some(SurfaceFormat::R8G8B8A8Srgb),
        DxgiFormat::B8G8R8A8_UNorm => Some(SurfaceFormat::B8G8R8A8Unorm),
        DxgiFormat::B8G8R8A8_UNorm_sRGB => Some(SurfaceFormat::B8G8R8A8Srgb),
        DxgiFormat::BC1_UNorm => Some(SurfaceFormat::BC1Unorm),
        DxgiFormat::BC1_UNorm_sRGB => Some(SurfaceFormat::BC1Srgb),
        DxgiFormat::BC2_UNorm => Some(SurfaceFormat::BC2Unorm),
        DxgiFormat::BC2_UNorm_sRGB => Some(SurfaceFormat::BC2Srgb),
        DxgiFormat::BC3_UNorm => Some(SurfaceFormat::BC3Unorm),
        DxgiFormat::BC3_UNorm_sRGB => Some(SurfaceFormat::BC3Srgb),
        DxgiFormat::BC4_UNorm => Some(SurfaceFormat::BC4Unorm),
        DxgiFormat::BC4_SNorm => Some(SurfaceFormat::BC4Snorm),
        DxgiFormat::BC5_UNorm => Some(SurfaceFormat::BC5Unorm),
        DxgiFormat::BC5_SNorm => Some(SurfaceFormat::BC5Snorm),
        DxgiFormat::BC6H_SF16 => Some(SurfaceFormat::BC6Sfloat),
        DxgiFormat::BC6H_UF16 => Some(SurfaceFormat::BC6Ufloat),
        DxgiFormat::BC7_UNorm => Some(SurfaceFormat::BC7Unorm),
        DxgiFormat::BC7_UNorm_sRGB => Some(SurfaceFormat::BC7Srgb),
        _ => None,
    }
}

fn image_format_from_d3d(format: D3DFormat) -> Option<SurfaceFormat> {
    // TODO: Support uncompressed formats.
    match format {
        D3DFormat::DXT1 => Some(SurfaceFormat::BC1Unorm),
        D3DFormat::DXT2 => Some(SurfaceFormat::BC2Unorm),
        D3DFormat::DXT3 => Some(SurfaceFormat::BC2Unorm),
        D3DFormat::DXT4 => Some(SurfaceFormat::BC3Unorm),
        D3DFormat::DXT5 => Some(SurfaceFormat::BC3Unorm),
        _ => None,
    }
}

const BC5U: u32 = u32::from_le_bytes(*b"BC5U");
const ATI2: u32 = u32::from_le_bytes(*b"ATI2");

fn image_format_from_fourcc(fourcc: &FourCC) -> Option<SurfaceFormat> {
    match fourcc.0 {
        FourCC::DXT1 => Some(SurfaceFormat::BC1Unorm),
        FourCC::DXT2 => Some(SurfaceFormat::BC2Unorm),
        FourCC::DXT3 => Some(SurfaceFormat::BC2Unorm),
        FourCC::DXT4 => Some(SurfaceFormat::BC3Unorm),
        FourCC::DXT5 => Some(SurfaceFormat::BC3Unorm),
        FourCC::BC4_UNORM => Some(SurfaceFormat::BC4Unorm),
        FourCC::BC4_SNORM => Some(SurfaceFormat::BC4Snorm),
        ATI2 | BC5U => Some(SurfaceFormat::BC5Unorm),
        FourCC::BC5_SNORM => Some(SurfaceFormat::BC5Snorm),
        _ => None,
    }
}

impl From<SurfaceFormat> for DxgiFormat {
    fn from(f: SurfaceFormat) -> Self {
        match f {
            SurfaceFormat::R8Unorm => Self::R8_UNorm,
            SurfaceFormat::R8G8B8A8Unorm => Self::R8G8B8A8_UNorm,
            SurfaceFormat::R8G8B8A8Srgb => Self::R8G8B8A8_UNorm_sRGB,
            SurfaceFormat::B8G8R8A8Unorm => Self::B8G8R8A8_UNorm,
            SurfaceFormat::B8G8R8A8Srgb => Self::B8G8R8A8_UNorm_sRGB,
            SurfaceFormat::BC1Unorm => Self::BC1_UNorm,
            SurfaceFormat::BC1Srgb => Self::BC1_UNorm_sRGB,
            SurfaceFormat::BC2Unorm => Self::BC2_UNorm,
            SurfaceFormat::BC2Srgb => Self::BC2_UNorm_sRGB,
            SurfaceFormat::BC3Unorm => Self::BC3_UNorm,
            SurfaceFormat::BC3Srgb => Self::BC3_UNorm_sRGB,
            SurfaceFormat::BC4Unorm => Self::BC4_UNorm,
            SurfaceFormat::BC4Snorm => Self::BC4_SNorm,
            SurfaceFormat::BC5Unorm => Self::BC5_UNorm,
            SurfaceFormat::BC5Snorm => Self::BC5_SNorm,
            SurfaceFormat::BC6Sfloat => Self::BC6H_SF16,
            SurfaceFormat::BC6Ufloat => Self::BC6H_UF16,
            SurfaceFormat::BC7Unorm => Self::BC7_UNorm,
            SurfaceFormat::BC7Srgb => Self::BC7_UNorm_sRGB,
        }
    }
}
