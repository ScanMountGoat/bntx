use std::convert::{TryFrom, TryInto};

use image_dds::{ddsfile::Dds, Surface};
use tegra_swizzle::{
    block_height_mip0, div_round_up, mip_block_height,
    surface::{swizzle_surface, BlockDim},
    BlockHeight,
};
use thiserror::Error;

use crate::{
    Bntx, BntxStr, Brtd, Brti, BrtiOffset, ByteOrder, DictNode, DictSection, Header, Mipmaps,
    NxHeader, RelocationEntry, RelocationSection, RelocationTable, StrSection, SurfaceFormat,
    TextureDimension, TextureViewDimension,
};

#[derive(Debug, Error)]
pub enum CreateBntxError {
    #[error("failed to swizzle surface")]
    SwizzleError(#[from] tegra_swizzle::SwizzleError),

    #[error("error creating surface")]
    Surface(#[from] image_dds::error::SurfaceError),

    #[error("unsupported format {0:?}")]
    UnsupportedImageFormat(image_dds::ImageFormat),
}

#[derive(Debug, Error)]
pub enum CreateDdsError {
    #[error("failed to swizzle surface")]
    SwizzleError(#[from] tegra_swizzle::SwizzleError),

    #[error("error creating surface")]
    Surface(#[from] CreateSurfaceError),

    #[error("error creating DDS")]
    Dds(#[from] image_dds::CreateDdsError),
}

#[derive(Debug, Error)]
pub enum CreateSurfaceError {
    #[error("failed to swizzle surface")]
    SwizzleError(#[from] tegra_swizzle::SwizzleError),

    #[error("unsupported format {0:?}")]
    UnsupportedSurfaceFormat(SurfaceFormat),
}

// Filled in during writing by xc3_write.
const TEMP_OFFSET: u32 = 0;

impl Bntx {
    pub fn to_surface(&self) -> Result<Surface<Vec<u8>>, CreateSurfaceError> {
        Ok(Surface {
            width: self.width(),
            height: self.height(),
            depth: self.depth(),
            layers: self.layer_count(),
            mipmaps: self.mipmap_count(),
            image_format: self.image_format().try_into()?,
            data: self.deswizzled_data()?,
        })
    }

    pub fn to_dds(&self) -> Result<Dds, CreateDdsError> {
        image_dds::Surface {
            height: self.height(),
            width: self.width(),
            depth: self.depth(),
            layers: self.layer_count(),
            mipmaps: self.mipmap_count(),
            image_format: self.image_format().try_into()?,
            data: self.deswizzled_data()?,
        }
        .to_dds()
        .map_err(Into::into)
    }

    pub fn from_surface<T: AsRef<[u8]>>(
        surface: Surface<T>,
        name: &str,
    ) -> Result<Self, CreateBntxError> {
        // Let tegra_swizzle calculate the block height.
        // This matches the value inferred for missing block heights like in nutexb.
        let format = SurfaceFormat::try_from(surface.image_format)?;
        let block_dim = format.block_dim();
        let block_height = block_height_mip0(div_round_up(surface.height, block_dim.height.get()));

        let block_height_log2 = match block_height {
            BlockHeight::One => 0,
            BlockHeight::Two => 1,
            BlockHeight::Four => 2,
            BlockHeight::Eight => 3,
            BlockHeight::Sixteen => 4,
            BlockHeight::ThirtyTwo => 5,
        };

        let bytes_per_pixel = format.bytes_per_pixel();

        let width = surface.width;
        let height = surface.height;
        let depth = surface.depth;
        let mipmap_count = surface.mipmaps;
        let layer_count = surface.layers;

        let data = swizzle_surface(
            width,
            height,
            depth,
            surface.data.as_ref(),
            block_dim,
            Some(block_height),
            bytes_per_pixel,
            mipmap_count,
            layer_count,
        )?;

        let str_section = StrSection {
            block_size: TEMP_OFFSET,
            block_offset: TEMP_OFFSET as u64,
            str_count: 1,
            empty: BntxStr::default(),
            strings: vec![BntxStr {
                chars: name.to_string(),
            }],
        };

        let mipmap_offsets = calculate_mipmap_offsets(
            mipmap_count,
            width,
            block_dim,
            height,
            depth,
            block_height,
            bytes_per_pixel,
        );

        Ok(Self {
            unk: 0,
            version: (0, 4),
            bom: ByteOrder::LittleEndian,
            header: Header {
                revision: 0x400c,
                file_name: TEMP_OFFSET,
                unk: 0,
                str_section,
                // TODO: how to initialize this data?
                // TODO: avoid hard coding offsets.
                reloc_table: RelocationTable {
                    position: TEMP_OFFSET,
                    count: 2,
                    unk1: 0,
                    sections: vec![
                        // Data until end of BRTIs
                        RelocationSection {
                            pointer: 0,
                            position: TEMP_OFFSET,
                            size: TEMP_OFFSET,
                            index: 0,
                            count: 4,
                        },
                        // BRTD to _RLT
                        RelocationSection {
                            pointer: 0,
                            position: TEMP_OFFSET,
                            size: TEMP_OFFSET,
                            index: 4,
                            count: 1,
                        },
                    ],
                    entries: vec![
                        // Section 0
                        RelocationEntry {
                            position: TEMP_OFFSET,
                            struct_count: 2,
                            offset_count: 1,
                            padding_count: 45,
                        },
                        RelocationEntry {
                            position: TEMP_OFFSET,
                            struct_count: 2,
                            offset_count: 2,
                            padding_count: 70,
                        },
                        RelocationEntry {
                            position: TEMP_OFFSET,
                            struct_count: 2,
                            offset_count: 1,
                            padding_count: 1,
                        },
                        RelocationEntry {
                            position: TEMP_OFFSET,
                            struct_count: 1,
                            offset_count: 3,
                            padding_count: 0,
                        },
                        // Section 1
                        RelocationEntry {
                            position: TEMP_OFFSET,
                            struct_count: 2,
                            offset_count: 1,
                            padding_count: 140,
                        },
                    ],
                },
                file_size: TEMP_OFFSET,
            },
            nx_header: NxHeader {
                brtis: vec![BrtiOffset {
                    brti: Brti {
                        size: 3576,
                        size2: 3576,
                        flags: 1,
                        texture_dimension: if depth > 1 {
                            TextureDimension::D3
                        } else {
                            TextureDimension::D2
                        },
                        tile_mode: 0,
                        swizzle: 0,
                        mipmap_count: mipmap_count as u16,
                        multi_sample_count: 1,
                        image_format: format,
                        unk2: 32,
                        width,
                        height,
                        depth,
                        layer_count,
                        block_height_log2,
                        unk4: [65543, 0, 0, 0, 0, 0],
                        image_size: data.len() as u32,
                        align: 512,
                        comp_sel: 84148994,
                        texture_view_dimension: if depth > 1 {
                            TextureViewDimension::D3
                        } else if layer_count == 6 {
                            TextureViewDimension::Cube
                        } else {
                            TextureViewDimension::D2
                        },
                        name_addr: TEMP_OFFSET as u64,
                        parent_addr: 32,
                        mipmaps: Mipmaps { mipmap_offsets },
                        unk5: 0,
                        unk6: [0; 256],
                        unk7: [0; 256],
                        unk: [0; 4],
                    },
                }],
                brtd: Brtd { image_data: data },
                dict: DictSection {
                    node_count: 1,
                    nodes: vec![
                        DictNode {
                            reference: -1,
                            left_index: 1,
                            right_index: 0,
                            name_offset: 436,
                        },
                        DictNode {
                            reference: 0, // TODO: 0 or 1?
                            left_index: 0,
                            right_index: 1,
                            name_offset: 440,
                        },
                    ],
                },
                dict_size: 88,
                unk: [0; 42],
            },
        })
    }

    pub fn from_dds(dds: &Dds, name: &str) -> Result<Self, CreateBntxError> {
        let surface = image_dds::Surface::from_dds(dds)?;
        Self::from_surface(surface, name)
    }
}

// TODO: Don't hard code these values.
const BRTD_SECTION_START: usize = 0xFF0;
const SIZE_OF_BRTD: usize = 0x10;
const START_OF_TEXTURE_DATA: usize = BRTD_SECTION_START + SIZE_OF_BRTD;

fn calculate_mipmap_offsets(
    mipmap_count: u32,
    width: u32,
    block_dim: BlockDim,
    height: u32,
    depth: u32,
    block_height: BlockHeight,
    bytes_per_pixel: u32,
) -> Vec<u64> {
    let mut mipmap_offsets = Vec::new();

    let mut mipmap_offset = 0;
    for mip in 0..mipmap_count {
        mipmap_offsets.push(START_OF_TEXTURE_DATA as u64 + mipmap_offset as u64);

        let mip_width = div_round_up((width >> mip).max(1), block_dim.width.get());
        let mip_height = div_round_up((height >> mip).max(1), block_dim.height.get());
        let mip_depth = div_round_up((depth >> mip).max(1), block_dim.depth.get());
        let mip_block_height = mip_block_height(mip_height, block_height);
        let mip_size = tegra_swizzle::swizzle::swizzled_mip_size(
            mip_width,
            mip_height,
            mip_depth,
            mip_block_height,
            bytes_per_pixel,
        );

        mipmap_offset += mip_size;
    }
    mipmap_offsets
}

impl TryFrom<SurfaceFormat> for image_dds::ImageFormat {
    type Error = CreateSurfaceError;

    fn try_from(value: SurfaceFormat) -> Result<Self, Self::Error> {
        // TODO: Add support to image_dds for remaining formats
        match value {
            SurfaceFormat::R8Unorm => Ok(Self::R8Unorm),
            SurfaceFormat::Unk1 => Err(CreateSurfaceError::UnsupportedSurfaceFormat(value)),
            SurfaceFormat::R8G8B8A8Unorm => Ok(Self::Rgba8Unorm),
            SurfaceFormat::R8G8B8A8Srgb => Ok(Self::Rgba8UnormSrgb),
            SurfaceFormat::B8G8R8A8Unorm => Ok(Self::Bgra8Unorm),
            SurfaceFormat::B8G8R8A8Srgb => Ok(Self::Bgra8UnormSrgb),
            SurfaceFormat::R11G11B10 => Err(CreateSurfaceError::UnsupportedSurfaceFormat(value)),
            SurfaceFormat::BC1Unorm => Ok(Self::BC1RgbaUnorm),
            SurfaceFormat::BC1Srgb => Ok(Self::BC1RgbaUnormSrgb),
            SurfaceFormat::BC2Unorm => Ok(Self::BC2RgbaUnorm),
            SurfaceFormat::BC2Srgb => Ok(Self::BC2RgbaUnormSrgb),
            SurfaceFormat::BC3Unorm => Ok(Self::BC3RgbaUnorm),
            SurfaceFormat::BC3Srgb => Ok(Self::BC3RgbaUnormSrgb),
            SurfaceFormat::BC4Unorm => Ok(Self::BC4RUnorm),
            SurfaceFormat::BC4Snorm => Ok(Self::BC4RSnorm),
            SurfaceFormat::BC5Unorm => Ok(Self::BC5RgUnorm),
            SurfaceFormat::BC5Snorm => Ok(Self::BC5RgSnorm),
            SurfaceFormat::BC6Sfloat => Ok(Self::BC6hRgbSfloat),
            SurfaceFormat::BC6Ufloat => Ok(Self::BC6hRgbUfloat),
            SurfaceFormat::BC7Unorm => Ok(Self::BC7RgbaUnorm),
            SurfaceFormat::BC7Srgb => Ok(Self::BC7RgbaUnormSrgb),
        }
    }
}

impl TryFrom<image_dds::ImageFormat> for SurfaceFormat {
    type Error = CreateBntxError;

    fn try_from(value: image_dds::ImageFormat) -> Result<Self, Self::Error> {
        match value {
            image_dds::ImageFormat::R8Unorm => Ok(Self::R8Unorm),
            image_dds::ImageFormat::Rgba8Unorm => Ok(Self::R8G8B8A8Unorm),
            image_dds::ImageFormat::Rgba8UnormSrgb => Ok(Self::R8G8B8A8Srgb),
            image_dds::ImageFormat::Rgba16Float => {
                Err(CreateBntxError::UnsupportedImageFormat(value))
            }
            image_dds::ImageFormat::Rgba32Float => {
                Err(CreateBntxError::UnsupportedImageFormat(value))
            }
            image_dds::ImageFormat::Bgra8Unorm => Ok(Self::B8G8R8A8Unorm),
            image_dds::ImageFormat::Bgra8UnormSrgb => Ok(Self::B8G8R8A8Srgb),
            image_dds::ImageFormat::Bgra4Unorm => {
                Err(CreateBntxError::UnsupportedImageFormat(value))
            }
            image_dds::ImageFormat::BC1RgbaUnorm => Ok(Self::BC1Unorm),
            image_dds::ImageFormat::BC1RgbaUnormSrgb => Ok(Self::BC1Srgb),
            image_dds::ImageFormat::BC2RgbaUnorm => Ok(Self::BC2Unorm),
            image_dds::ImageFormat::BC2RgbaUnormSrgb => Ok(Self::BC2Srgb),
            image_dds::ImageFormat::BC3RgbaUnorm => Ok(Self::BC3Unorm),
            image_dds::ImageFormat::BC3RgbaUnormSrgb => Ok(Self::BC3Srgb),
            image_dds::ImageFormat::BC4RUnorm => Ok(Self::BC4Unorm),
            image_dds::ImageFormat::BC4RSnorm => Ok(Self::BC4Snorm),
            image_dds::ImageFormat::BC5RgUnorm => Ok(Self::BC5Unorm),
            image_dds::ImageFormat::BC5RgSnorm => Ok(Self::BC5Snorm),
            image_dds::ImageFormat::BC6hRgbUfloat => Ok(Self::BC6Ufloat),
            image_dds::ImageFormat::BC6hRgbSfloat => Ok(Self::BC6Sfloat),
            image_dds::ImageFormat::BC7RgbaUnorm => Ok(Self::BC7Unorm),
            image_dds::ImageFormat::BC7RgbaUnormSrgb => Ok(Self::BC7Srgb),
            _ => Err(CreateBntxError::UnsupportedImageFormat(value)),
        }
    }
}
