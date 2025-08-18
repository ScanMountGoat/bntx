use binrw::{
    args, binread, binrw, BinRead, BinReaderExt, BinWrite, FilePtr16, FilePtr32, FilePtr64,
};
use std::convert::TryFrom;
use std::io::{Seek, Write};
use std::path::Path;
use tegra_swizzle::surface::BlockDim;
use xc3_write::{Endian, WriteFull, Xc3Write, Xc3WriteOffsets};

// TODO: Add module level docs for basic usage.
pub mod surface;

// TODO: Decompile syroot.nintentools.bntx from switch toolbox to figure out how writing works.
#[derive(Debug, BinRead, Xc3Write, PartialEq, Clone)]
#[br(magic = b"BNTX")]
#[xc3(magic(b"BNTX"))]
pub struct Bntx {
    // TODO: always 0?
    pub unk: u32,

    pub version: (u16, u16),

    pub bom: ByteOrder,

    #[br(is_little = bom == ByteOrder::LittleEndian)]
    pub header: Header,

    #[br(is_little = bom == ByteOrder::LittleEndian)]
    pub nx_header: NxHeader,
}

// Use byte literals to ignore reader endianness.
#[derive(Debug, BinRead, BinWrite, PartialEq, Clone, Copy)]
pub enum ByteOrder {
    #[brw(magic = b"\xFF\xFE")]
    LittleEndian,
    #[brw(magic = b"\xFE\xFF")]
    BigEndian,
}

#[binread]
#[derive(Debug, Xc3Write, PartialEq, Clone)]
pub struct Header {
    pub revision: u16,

    // TODO: Should shared offsets prevent deriving write offsets?
    #[xc3(shared_offset)]
    pub file_name: u32,

    pub unk: u16,

    #[br(parse_with = FilePtr16::parse)]
    #[xc3(offset(u16))]
    pub str_section: StrSection,

    // TODO: The last item in the file?
    #[br(parse_with = FilePtr32::parse)]
    #[xc3(offset(u32))]
    pub reloc_table: RelocationTable,

    // TODO: calculate this automatically?
    #[xc3(shared_offset)]
    pub file_size: u32,
}

// TODO: How to recalculate this when saving?
#[derive(Debug, BinRead, Xc3Write, PartialEq, Clone)]
#[br(magic(b"_RLT"))]
#[xc3(magic(b"_RLT"))]
pub struct RelocationTable {
    #[xc3(shared_offset)]
    pub position: u32,
    pub count: u32,
    pub unk1: u32, // 0

    // TODO: main header section and brtd?
    #[br(count = count)]
    pub sections: Vec<RelocationSection>,

    // TODO: Pointers to string pointers?
    #[br(count = sections.iter().map(|x| x.count).sum::<u32>())]
    pub entries: Vec<RelocationEntry>,
}

#[derive(Debug, BinRead, Xc3Write, PartialEq, Clone)]
pub struct RelocationSection {
    pub pointer: u64,
    #[xc3(shared_offset)]
    pub position: u32,
    #[xc3(shared_offset)]
    pub size: u32,
    pub index: u32,
    pub count: u32,
}

#[derive(Debug, BinRead, Xc3Write, PartialEq, Clone)]
pub struct RelocationEntry {
    #[xc3(shared_offset)]
    pub position: u32,
    pub struct_count: u16,
    pub offset_count: u8,
    pub padding_count: u8,
}

#[derive(Debug, BinRead, Xc3Write, PartialEq, Clone)]
#[br(magic(b"_STR"))]
#[xc3(magic(b"_STR"))]
pub struct StrSection {
    #[xc3(shared_offset)]
    pub block_size: u32,
    #[xc3(shared_offset)]
    pub block_offset: u64,

    pub str_count: u32,

    pub empty: BntxStr,

    #[br(count = str_count)]
    #[xc3(align_after = 8)]
    pub strings: Vec<BntxStr>,
}

// TODO: These all refer to the string dict?
#[binrw]
#[derive(Debug, PartialEq, Clone, Default)]
pub struct BntxStr {
    #[br(temp)]
    #[bw(calc = chars.len() as u16)]
    len: u16,

    #[brw(pad_after = 1, align_after = 2)]
    #[br(count = len, map = |x: Vec<u8>| String::from_utf8_lossy(&x).into_owned())]
    #[bw(map = |s| s.as_bytes())]
    pub chars: String,
}

#[binread]
#[derive(Debug, Xc3Write, Xc3WriteOffsets, PartialEq, Clone)]
#[br(magic = b"NX  ")]
#[xc3(magic(b"NX  "))]
pub struct NxHeader {
    #[br(temp)]
    count: u32,

    #[br(parse_with = FilePtr64::parse)]
    #[br(args { inner: args! { count: count as usize } })]
    #[xc3(count_offset(u32, u64))]
    pub brtis: Vec<BrtiOffset>,

    #[br(parse_with = FilePtr64::parse)]
    #[xc3(offset(u64))]
    pub brtd: Brtd,

    #[br(parse_with = FilePtr64::parse)]
    #[xc3(offset(u64))]
    pub dict: DictSection,
    // TODO: How to calculate this
    pub dict_size: u64,

    // TODO: 336 bytes of padding?
    pub unk: [u64; 42],
}

#[derive(Debug, BinRead, Xc3Write, Xc3WriteOffsets, PartialEq, Clone)]
pub struct BrtiOffset {
    #[br(parse_with = FilePtr64::parse)]
    #[xc3(offset(u64))]
    pub brti: Brti,
}

#[derive(Debug, BinRead, BinWrite, PartialEq, Clone)]
#[brw(magic = b"_DIC")]
pub struct DictSection {
    pub node_count: u32,
    // TODO: some sort of root node is always included?
    #[br(count = node_count + 1)]
    pub nodes: Vec<DictNode>,
}

#[derive(Debug, BinRead, BinWrite, PartialEq, Clone)]
pub struct DictNode {
    pub reference: i32,
    pub left_index: u16,
    pub right_index: u16,
    // TODO: How to correctly calculate this offset?
    pub name_offset: u64,
}

// TODO: Are these flags?
#[derive(Debug, BinRead, BinWrite, Clone, Copy, PartialEq, Eq)]
#[brw(repr(u32))]
pub enum SurfaceFormat {
    R8Unorm = 0x0201,
    Unk1 = 0x0a05,
    R8G8B8A8Unorm = 0x0b01,
    R8G8B8A8Srgb = 0x0b06,
    B8G8R8A8Unorm = 0x0c01,
    B8G8R8A8Srgb = 0x0c06,
    R11G11B10 = 0x0f05,
    BC1Unorm = 0x1a01,
    BC1Srgb = 0x1a06,
    BC2Unorm = 0x1b01,
    BC2Srgb = 0x1b06,
    BC3Unorm = 0x1c01,
    BC3Srgb = 0x1c06,
    BC4Unorm = 0x1d01,
    BC4Snorm = 0x1d02,
    BC5Unorm = 0x1e01,
    BC5Snorm = 0x1e02,
    BC6Sfloat = 0x1f05,
    BC6Ufloat = 0x1f0a,
    BC7Unorm = 0x2001,
    BC7Srgb = 0x2006,
    // TODO: Fill in other known formats.
}

#[derive(Debug, BinRead, Xc3Write, Xc3WriteOffsets, PartialEq, Clone)]
#[br(magic = b"BRTI")]
#[xc3(magic(b"BRTI"))]
pub struct Brti {
    #[xc3(shared_offset)]
    pub size: u32, // offset?
    #[xc3(shared_offset)]
    pub size2: u64, // size?
    pub flags: u8,
    pub texture_dimension: TextureDimension,
    pub tile_mode: u16,
    pub swizzle: u16,
    pub mipmap_count: u16,
    pub multi_sample_count: u32,
    pub image_format: SurfaceFormat,
    pub unk2: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub layer_count: u32,
    pub block_height_log2: u32,
    pub unk4: [u32; 6],  // TODO: What is this?
    pub image_size: u32, // the total size of all layers and mipmaps with padding
    pub align: u32, // usually 512 to match the expected mipmap alignment for swizzled surfaces.
    pub comp_sel: u32,
    pub texture_view_dimension: TextureViewDimension,

    // TODO: This should point into the string section.
    // TODO: pointer to bntx string for name
    #[xc3(shared_offset)]
    pub name_addr: u64,

    #[xc3(shared_offset)]
    pub parent_addr: u64, // TODO: pointer to nx header

    // TODO: This is a pointer to an array of u64 mipmap offsets.
    // TODO: Parse the entire surface in one vec but store the mipmap offsets?
    #[br(parse_with = FilePtr64::parse, args { inner: (mipmap_count,)})]
    #[xc3(offset(u64))]
    pub mipmaps: Mipmaps,

    pub unk5: u64, // always 0?

    // TODO: always 0?
    #[br(parse_with = FilePtr64::parse)]
    #[xc3(offset(u64))]
    pub unk6: [u8; 256],

    #[br(parse_with = FilePtr64::parse)]
    #[xc3(offset(u64))]
    pub unk7: [u8; 256],

    // TODO: padding?
    pub unk: [u32; 4],
}

#[derive(Debug, BinRead, BinWrite, Clone, Copy, PartialEq, Eq)]
#[brw(repr(u8))]
pub enum TextureDimension {
    D1 = 1,
    D2 = 2,
    D3 = 3,
}

#[derive(Debug, BinRead, BinWrite, Clone, Copy, PartialEq, Eq)]
#[brw(repr(u32))]
pub enum TextureViewDimension {
    D1 = 0,
    D2 = 1,
    D3 = 2,
    Cube = 3,
    // TODO: Fill in other known variants
}

#[binrw]
#[brw(magic = b"BRTD")]
#[derive(Debug, PartialEq, Clone)]
pub struct Brtd {
    // Size of the image data + BRTD header.
    #[brw(pad_before = 4)]
    #[br(temp)]
    #[bw(calc = image_data.len() as u64 + 16)]
    brtd_size: u64,

    #[br(count = brtd_size - 16)]
    pub image_data: Vec<u8>,
}

#[derive(Debug, BinRead, BinWrite, PartialEq, Clone)]
#[br(import(mipmap_count: u16))]
pub struct Mipmaps {
    #[br(count = mipmap_count)]
    pub mipmap_offsets: Vec<u64>,
}

impl Bntx {
    pub fn width(&self) -> u32 {
        self.nx_header.brtis[0].brti.width
    }

    pub fn height(&self) -> u32 {
        self.nx_header.brtis[0].brti.height
    }

    pub fn depth(&self) -> u32 {
        self.nx_header.brtis[0].brti.depth
    }

    pub fn layer_count(&self) -> u32 {
        self.nx_header.brtis[0].brti.layer_count
    }

    pub fn mipmap_count(&self) -> u32 {
        self.nx_header.brtis[0].brti.mipmap_count as u32
    }

    pub fn image_format(&self) -> SurfaceFormat {
        self.nx_header.brtis[0].brti.image_format
    }

    /// The deswizzled image data for all layers and mipmaps.
    pub fn deswizzled_data(&self) -> Result<Vec<u8>, tegra_swizzle::SwizzleError> {
        let info = &self.nx_header.brtis[0].brti;

        tegra_swizzle::surface::deswizzle_surface(
            info.width,
            info.height,
            info.depth,
            &self.nx_header.brtd.image_data,
            info.image_format.block_dim(),
            None, // TODO: use block height from header?
            info.image_format.bytes_per_pixel(),
            info.mipmap_count as u32,
            info.layer_count,
        )
    }
    // TODO: from_image_data?
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, binrw::error::Error> {
        let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
        reader.read_le()
    }

    pub fn write<W: Write + Seek>(&self, writer: &mut W) -> std::io::Result<()> {
        self.write_full(writer, 0, &mut 0, Endian::Little, ())
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let mut writer = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        self.write(&mut writer)
    }
}

impl<'a> Xc3WriteOffsets for BntxOffsets<'a> {
    type Args = ();

    fn write_offsets<W: Write + Seek>(
        &self,
        writer: &mut W,
        base_offset: u64,
        data_ptr: &mut u64,
        endian: Endian,
        _args: Self::Args,
    ) -> std::io::Result<()> {
        // Match the convention for ordering of data items in bntx files.
        let brtis = self
            .nx_header
            .brtis
            .write(writer, base_offset, data_ptr, endian)?;

        // TODO: Add an attribute for storing positions of fields and types?
        let str_section_pos = *data_ptr;
        let str_section = self
            .header
            .str_section
            .write(writer, base_offset, data_ptr, endian)?;

        // Point to the string chars.
        self.header
            .file_name
            .set_offset(writer, str_section_pos + 26, endian)?;

        let dict_pos = data_ptr.next_multiple_of(8);
        self.nx_header
            .dict
            .write_full(writer, base_offset, data_ptr, endian, ())?;

        // TODO: why does the str section point past the dict section?
        str_section
            .block_offset
            .set_offset(writer, *data_ptr - str_section_pos, endian)?;
        str_section
            .block_size
            .set_offset(writer, *data_ptr - str_section_pos, endian)?;

        let brtis_pos = *data_ptr;
        for brti in brtis.0 {
            let brti_position = *data_ptr;
            let brti = brti.brti.write(writer, base_offset, data_ptr, endian)?;

            // TODO: How to set this if there is more than 1 BRTI?
            brti.size.set_offset(writer, 4080 - brti_position, endian)?;
            brti.size2
                .set_offset(writer, 4080 - brti_position, endian)?;

            // Point to the bntx string.
            brti.name_addr
                .set_offset(writer, str_section_pos + 24, endian)?;

            // TODO: nx address?
            brti.parent_addr.set_offset(writer, 32, endian)?;

            brti.unk6
                .write_full(writer, base_offset, data_ptr, endian, ())?;
            brti.unk7
                .write_full(writer, base_offset, data_ptr, endian, ())?;
            brti.mipmaps
                .write_full(writer, base_offset, data_ptr, endian, ())?;
        }
        let after_brti_pos = *data_ptr;

        // TODO: Is this fixed padding?
        *data_ptr = 4080;
        self.nx_header
            .brtd
            .write_full(writer, base_offset, data_ptr, endian, ())?;

        let reloc_table_pos = *data_ptr;
        let reloc_table = self
            .header
            .reloc_table
            .write(writer, base_offset, data_ptr, endian)?;
        reloc_table
            .position
            .set_offset(writer, reloc_table_pos, endian)?;

        // Data until end of BRTIs
        reloc_table.sections.0[0]
            .size
            .set_offset(writer, after_brti_pos, endian)?;

        // BRTD to _RLT
        reloc_table.sections.0[1]
            .position
            .set_offset(writer, 4080, endian)?;
        reloc_table.sections.0[1].size.set_offset(
            writer,
            self.nx_header.brtd.data.image_data.len() as u64 + 16,
            endian,
        )?;

        // TODO: How to set the padding?
        // _RLT Section 0
        reloc_table.entries.0[0]
            .position
            .set_offset(writer, 40, endian)?;
        reloc_table.entries.0[1]
            .position
            .set_offset(writer, 56, endian)?;
        // _DIC str offsets
        reloc_table.entries.0[2]
            .position
            .set_offset(writer, dict_pos + 16, endian)?;
        // _BRTI str offset
        reloc_table.entries.0[3]
            .position
            .set_offset(writer, brtis_pos + 96, endian)?;

        // _RLT Section 1
        // BRTD offset
        reloc_table.entries.0[4]
            .position
            .set_offset(writer, 48, endian)?;

        // This fills in the file size since we write it last.
        self.header
            .file_size
            .write_full(writer, base_offset, data_ptr, endian, ())?;
        Ok(())
    }
}

impl SurfaceFormat {
    fn bytes_per_pixel(&self) -> u32 {
        match self {
            SurfaceFormat::R8Unorm => 1,
            SurfaceFormat::Unk1 => todo!(),
            SurfaceFormat::R8G8B8A8Unorm => 4,
            SurfaceFormat::R8G8B8A8Srgb => 4,
            SurfaceFormat::B8G8R8A8Unorm => 4,
            SurfaceFormat::B8G8R8A8Srgb => 4,
            SurfaceFormat::R11G11B10 => 4,
            SurfaceFormat::BC1Unorm => 8,
            SurfaceFormat::BC1Srgb => 8,
            SurfaceFormat::BC2Unorm => 16,
            SurfaceFormat::BC2Srgb => 16,
            SurfaceFormat::BC3Unorm => 16,
            SurfaceFormat::BC3Srgb => 16,
            SurfaceFormat::BC4Unorm => 8,
            SurfaceFormat::BC4Snorm => 8,
            SurfaceFormat::BC5Unorm => 16,
            SurfaceFormat::BC5Snorm => 16,
            SurfaceFormat::BC6Sfloat => 16,
            SurfaceFormat::BC6Ufloat => 16,
            SurfaceFormat::BC7Unorm => 16,
            SurfaceFormat::BC7Srgb => 16,
        }
    }

    fn block_dim(&self) -> BlockDim {
        match self {
            SurfaceFormat::R8Unorm => BlockDim::uncompressed(),
            SurfaceFormat::Unk1 => todo!(),
            SurfaceFormat::R8G8B8A8Unorm => BlockDim::uncompressed(),
            SurfaceFormat::R8G8B8A8Srgb => BlockDim::uncompressed(),
            SurfaceFormat::B8G8R8A8Unorm => BlockDim::uncompressed(),
            SurfaceFormat::B8G8R8A8Srgb => BlockDim::uncompressed(),
            SurfaceFormat::R11G11B10 => BlockDim::uncompressed(),
            SurfaceFormat::BC1Unorm => BlockDim::block_4x4(),
            SurfaceFormat::BC1Srgb => BlockDim::block_4x4(),
            SurfaceFormat::BC2Unorm => BlockDim::block_4x4(),
            SurfaceFormat::BC2Srgb => BlockDim::block_4x4(),
            SurfaceFormat::BC3Unorm => BlockDim::block_4x4(),
            SurfaceFormat::BC3Srgb => BlockDim::block_4x4(),
            SurfaceFormat::BC4Unorm => BlockDim::block_4x4(),
            SurfaceFormat::BC4Snorm => BlockDim::block_4x4(),
            SurfaceFormat::BC5Unorm => BlockDim::block_4x4(),
            SurfaceFormat::BC5Snorm => BlockDim::block_4x4(),
            SurfaceFormat::BC6Sfloat => BlockDim::block_4x4(),
            SurfaceFormat::BC6Ufloat => BlockDim::block_4x4(),
            SurfaceFormat::BC7Unorm => BlockDim::block_4x4(),
            SurfaceFormat::BC7Srgb => BlockDim::block_4x4(),
        }
    }
}

macro_rules! xc3_write_binwrite_impl {
    ($($ty:ty),*) => {
        $(
            impl Xc3Write for $ty {
                // This also enables write_full since () implements Xc3WriteOffsets.
                type Offsets<'a> = ();

                fn xc3_write<W: std::io::Write + std::io::Seek>(
                    &self,
                    writer: &mut W,
                    endian: xc3_write::Endian,
                ) -> xc3_write::Xc3Result<Self::Offsets<'_>> {
                    let endian = match endian {
                        xc3_write::Endian::Little => binrw::Endian::Little,
                        xc3_write::Endian::Big => binrw::Endian::Big,
                    };
                    self.write_options(writer, endian, ()).map_err(std::io::Error::other)?;
                    Ok(())
                }

                // TODO: Should this be specified manually?
                const ALIGNMENT: u64 = std::mem::align_of::<$ty>() as u64;
            }
        )*

    };
}

xc3_write_binwrite_impl!(
    Brtd,
    ByteOrder,
    TextureDimension,
    TextureViewDimension,
    SurfaceFormat,
    BntxStr,
    DictSection,
    Mipmaps
);

#[cfg(test)]
mod tests {
    // TODO: test cases?
}
