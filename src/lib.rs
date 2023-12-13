use binrw::{
    binread, binrw, BinRead, BinReaderExt, BinResult, BinWrite, FilePtr16, FilePtr32, FilePtr64,
};
use std::convert::TryFrom;
use std::io::{Seek, Write};
use std::path::Path;
use tegra_swizzle::surface::{deswizzle_surface, BlockDim};
use tegra_swizzle::BlockHeight;
use xc3_write::{write_full, xc3_write_binwrite_impl, Xc3Write, Xc3WriteOffsets};

// TODO: Add module level docs for basic usage.
// TODO: Make this optional.
pub mod dds;

// TODO: add an option to delay calling xc3_write on inner types?
// TODO: manually implement write offsets to control ordering.
// TODO: Decompile syroot.nintentools.bntx from switch toolbox to figure out how writing works.
#[derive(Debug, BinRead, Xc3Write)]
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
#[derive(Debug, Xc3Write, Xc3WriteOffsets)]
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
#[binrw]
#[derive(Debug)]
#[brw(magic = b"_RLT")]
#[bw(stream = w)]
pub struct RelocationTable {
    #[br(temp)]
    #[bw(calc = w.stream_position().unwrap() as u32 - 4)]
    pub rlt_section_pos: u32,

    #[br(temp)]
    #[bw(calc = sections.len() as u32)]
    pub count: u32,

    // TODO: main header section and brtd?
    #[br(pad_before = 4, count = count)]
    #[bw(pad_before = 4)]
    pub sections: Vec<RelocationSection>,

    // TODO: Pointers to string pointers?
    #[br(count = sections.iter().map(|x| x.count).sum::<u32>())]
    pub entries: Vec<RelocationEntry>,
}

#[derive(Debug, BinRead, BinWrite)]
pub struct RelocationSection {
    pub pointer: u64,
    pub position: u32,
    pub size: u32,
    pub index: u32,
    pub count: u32,
}

#[derive(Debug, BinRead, BinWrite)]
pub struct RelocationEntry {
    pub position: u32,
    pub struct_count: u16,
    pub offset_count: u8,
    pub padding_count: u8,
}

// TODO: Find a way to get offsets from strings?
#[binrw]
#[derive(Debug)]
#[brw(magic = b"_STR")]
pub struct StrSection {
    pub block_size: u32,
    pub block_offset: u64,

    #[bw(calc = strings.len() as u32)]
    pub str_count: u32,

    // #[br(temp)]
    #[bw(calc = BntxStr::default())]
    pub empty: BntxStr,

    #[br(count = str_count)]
    #[bw(align_after = 8)]
    pub strings: Vec<BntxStr>,
}

// TODO: These all refer to the string dict?
#[binrw]
#[derive(Debug, Default)]
pub struct BntxStr {
    #[br(temp)]
    #[bw(calc = chars.len() as u16)]
    len: u16,

    #[br(align_after = 4, count = len, map = |x: Vec<u8>| String::from_utf8_lossy(&x).into_owned())]
    #[bw(align_after = 4, map = |s| bytes_null_terminated(s))]
    pub chars: String,
}

fn bytes_null_terminated(s: &str) -> Vec<u8> {
    let mut bytes = s.as_bytes().to_vec();
    bytes.push(0u8);
    bytes
}

// TODO: Fields written in a special order?
#[binread]
#[derive(Debug, Xc3Write, Xc3WriteOffsets)]
#[br(magic = b"NX  ")]
#[xc3(magic(b"NX  "))]
pub struct NxHeader {
    // TODO: Is this an array?
    pub count: u32,
    #[br(parse_with = FilePtr64::parse)]
    #[xc3(offset(u64))]
    pub brti: BrtiOffset,

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

#[derive(Debug, BinRead, Xc3Write, Xc3WriteOffsets)]
pub struct BrtiOffset {
    #[br(parse_with = FilePtr64::parse)]
    #[xc3(offset(u64))]
    pub brti: Brti,
}

#[derive(Debug, BinRead, BinWrite)]
#[brw(magic = b"_DIC")]
pub struct DictSection {
    pub node_count: u32,
    // TODO: some sort of root node is always included?
    #[br(count = node_count + 1)]
    pub nodes: Vec<DictNode>,
}

#[derive(Debug, BinRead, BinWrite)]
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
    R8G8B8A8Unorm = 0x0b01,
    R8G8B8A8Srgb = 0x0b06,
    B8G8R8A8Unorm = 0x0c01,
    B8G8R8A8Srgb = 0x0c06,
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

#[derive(Debug, BinRead, Xc3Write, Xc3WriteOffsets)]
#[br(magic = b"BRTI")]
#[xc3(magic(b"BRTI"))]
pub struct Brti {
    pub size: u32,  // offset?
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
#[derive(Debug)]
pub struct Brtd {
    // Size of the image data + BRTD header.
    #[brw(pad_before = 4)]
    #[br(temp)]
    #[bw(calc = image_data.len() as u64 + 16)]
    brtd_size: u64,

    #[br(count = brtd_size - 16)]
    pub image_data: Vec<u8>,
}

#[derive(Debug, BinRead, BinWrite)]
#[br(import(mipmap_count: u16))]
pub struct Mipmaps {
    #[br(count = mipmap_count)]
    pub mipmap_offsets: Vec<u64>,
}

xc3_write_binwrite_impl!(
    Brtd,
    ByteOrder,
    StrSection,
    TextureDimension,
    TextureViewDimension,
    SurfaceFormat,
    BntxStr,
    DictSection,
    Mipmaps,
    RelocationTable
);

impl Bntx {
    pub fn width(&self) -> u32 {
        self.nx_header.brti.brti.width
    }

    pub fn height(&self) -> u32 {
        self.nx_header.brti.brti.height
    }

    pub fn depth(&self) -> u32 {
        self.nx_header.brti.brti.depth
    }

    pub fn layer_count(&self) -> u32 {
        self.nx_header.brti.brti.layer_count
    }

    pub fn mipmap_count(&self) -> u32 {
        self.nx_header.brti.brti.mipmap_count as u32
    }

    pub fn image_format(&self) -> SurfaceFormat {
        self.nx_header.brti.brti.image_format
    }

    /// The deswizzled image data for all layers and mipmaps.
    pub fn deswizzled_data(&self) -> Result<Vec<u8>, tegra_swizzle::SwizzleError> {
        let info = &self.nx_header.brti.brti;

        deswizzle_surface(
            info.width as usize,
            info.height as usize,
            info.depth as usize,
            &self.nx_header.brtd.image_data,
            info.image_format.block_dim(),
            Some(BlockHeight::new(2u32.pow(info.block_height_log2) as usize).unwrap()),
            info.image_format.bytes_per_pixel(),
            info.mipmap_count as usize,
            info.layer_count as usize,
        )
    }
    // TODO: from_image_data?
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, binrw::error::Error> {
        let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
        reader.read_le()
    }

    pub fn write<W: Write + Seek>(&self, writer: &mut W) -> Result<(), binrw::error::Error> {
        write_full(self, writer, 0, &mut 0)
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), binrw::error::Error> {
        let mut writer = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        self.write(&mut writer)
    }
}

impl<'a> Xc3WriteOffsets for BntxOffsets<'a> {
    fn write_offsets<W: Write + Seek>(
        &self,
        writer: &mut W,
        base_offset: u64,
        data_ptr: &mut u64,
    ) -> BinResult<()> {
        // Match the convention for ordering of data items in bntx files.
        let brti = self
            .nx_header
            .brti
            .write_offset(writer, base_offset, data_ptr)?;

        // TODO: Add an attribute for storing positions of fields and types?
        let str_section_pos = *data_ptr;
        self.header
            .str_section
            .write_full(writer, base_offset, data_ptr)?;

        // TODO: Create a unique type for shared offsets that doesn't take &mut?
        // TODO: Store the position of the string section.
        // Point to the string chars.
        self.header
            .file_name
            .write_full(writer, base_offset, &mut (str_section_pos + 26))?;

        self.nx_header
            .dict
            .write_full(writer, base_offset, data_ptr)?;

        let brti = brti.brti.write_offset(writer, base_offset, data_ptr)?;
        // Point to the bntx string.
        brti.name_addr
            .write_full(writer, base_offset, &mut (str_section_pos + 24))?;

        // TODO: nx address?
        brti.parent_addr
            .write_full(writer, base_offset, &mut (32))?;

        brti.unk6.write_full(writer, base_offset, data_ptr)?;
        brti.unk7.write_full(writer, base_offset, data_ptr)?;
        brti.mipmaps.write_full(writer, base_offset, data_ptr)?;

        // TODO: Is this fixed padding?
        *data_ptr = 4080;
        self.nx_header
            .brtd
            .write_full(writer, base_offset, data_ptr)?;

        self.header
            .reloc_table
            .write_full(writer, base_offset, data_ptr)?;

        // This fills in the file size since we write it last.
        self.header
            .file_size
            .write_full(writer, base_offset, data_ptr)?;
        Ok(())
    }
}

impl SurfaceFormat {
    fn bytes_per_pixel(&self) -> usize {
        match self {
            SurfaceFormat::R8Unorm => 1,
            SurfaceFormat::R8G8B8A8Unorm => 4,
            SurfaceFormat::R8G8B8A8Srgb => 4,
            SurfaceFormat::B8G8R8A8Unorm => 4,
            SurfaceFormat::B8G8R8A8Srgb => 4,
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
            SurfaceFormat::R8G8B8A8Unorm => BlockDim::uncompressed(),
            SurfaceFormat::R8G8B8A8Srgb => BlockDim::uncompressed(),
            SurfaceFormat::B8G8R8A8Unorm => BlockDim::uncompressed(),
            SurfaceFormat::B8G8R8A8Srgb => BlockDim::uncompressed(),
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

#[cfg(test)]
mod tests {
    // TODO: test cases?
}
