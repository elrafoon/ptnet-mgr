use clap::{Parser, Subcommand, Args};
use ptnet::image_header::{self};
use ptnet::helpers::{any_as_u8_slice_mut, any_as_u8_slice};
use std::io::{Seek, BufWriter, Write, SeekFrom};
use std::str::FromStr;
use std::{path::{PathBuf}, fs::File, mem::size_of, io::{BufReader, Read}};

#[derive(Parser,Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands
}


#[derive(Subcommand,Debug)]
enum Commands {
    Add(AddHeader),
    Print(PrintHeader)
}

#[derive(Args,Debug)]
struct AddHeader {
    /// input file
    #[arg(short,long="in")]
    infile: PathBuf,
    /// output file
    #[arg(short,long="out")]
    outfile: PathBuf,
    /// hardware version vid:pid:rev
    #[arg(long)]
    hw: String,
    /// firmware version major.minor.patch
    #[arg(long)]
    fw: String
}

#[derive(Args,Debug)]
struct PrintHeader {
   /// input file
   #[arg(short,long="in")]
   infile: PathBuf
}


#[derive(Debug)]
enum Error {
    IOError(std::io::Error),
    LoadError(image_header::LoadError),
    ImageError(image_header::VerifyError),
    ParseError(image_header::ParseError)
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IOError(io_error) => { write!(f, "{}", io_error) },
            Error::LoadError(load_error) => { write!(f, "{}", load_error) },
            Error::ImageError(img_error) => { write!(f, "{}", img_error) },
            Error::ParseError(parse_error) => { write!(f, "{}", parse_error) }
        }
    }
}

impl std::error::Error for Error {
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self { Error::IOError(value) }
}

impl From<image_header::VerifyError> for Error {
    fn from(value: image_header::VerifyError) -> Self { Error::ImageError(value) }
}

impl From<image_header::ParseError> for Error {
    fn from(value: image_header::ParseError) -> Self { Error::ParseError(value) }
}

impl From<image_header::LoadError> for Error {
    fn from(value: image_header::LoadError) -> Self { Error::LoadError(value) }
}

fn print_header(params: &PrintHeader) -> Result<(), Error> {
    let fin = File::open(&params.infile)?;
    let (hdr, _payload) = image_header::Container::load_from(fin)?;
    println!("Header: {:?}", hdr);
    Ok(())
}

fn add_header(params: &AddHeader) -> Result<(), Error> {
    let fin = File::open(&params.infile)?;
    let mut pay: Vec<u8> = Vec::new();
    BufReader::new(fin).read_to_end(&mut pay)?;

    let mut hdr = image_header::Container::default();
    let fields = unsafe { &mut hdr.header.fields };
    fields.version = 0;
    fields.v0.hw_version = FromStr::from_str(&params.hw)?;
    fields.v0.fw_version = FromStr::from_str(&params.fw)?;
    fields.v0.payload_size = pay.len() as u32;
    fields.v0.payload_crc = image_header::crc(&pay[..]);
    hdr.header_crc = image_header::crc(unsafe { &hdr.header.raw });

    let fout = File::create(&params.outfile)?;
    let mut writer = BufWriter::new(fout);
    writer.write_all(&pay[..])?;
    writer.write_all(unsafe { any_as_u8_slice(&hdr) })?;

    Ok(())
}

fn main() -> Result<(), String> {
    let args = Cli::parse();

    let result = match &args.command {
        Commands::Add(params) => add_header(params),
        Commands::Print(params) => print_header(params)
    };

    match result {
        Result::Ok(_) => { Ok(()) },
        Result::Err(error) => { Err(format!("{}", error)) }
    }
}
