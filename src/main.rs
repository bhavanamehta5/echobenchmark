//echo benchmark for performance testing
use std::{collections::HashMap, ffi::CString, net::Ipv4Addr};
use catnip::{libos::LibOS, operations::OperationResult, protocols::ip::Port, protocols::ipv4::Ipv4Endpoint};
use demikernel::catnip::{dpdk::initialize_dpdk, memory::DPDKBuf, runtime::DPDKRuntime};
use anyhow::{Error, Result};
use clap::Parser;

/// Echo Benchmark
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Mode you want to run this machine on - client or server
    #[clap(short, long)]
    mode: String,

    /// server address
    #[clap(long, default_value_t = Ipv4Addr::new(10, 4, 0, 7))]
    server_address: Ipv4Addr,

    /// server port
    #[clap(long, default_value_t = 12345)]
    serverport_number: u16,
    
    /// client address
    #[clap(short, long, default_value_t = Ipv4Addr::new(10, 4, 0, 5))]
    client_address: Ipv4Addr,

    ///number of times client send data to the server
    #[clap(short, long, default_value_t =20)]
    loops: u8,

}

fn main() -> Result<(), Error>  
{
    let args = Args::parse();
    // extract the matches
    let serverport_number = args.serverport_number;
    let mode = args.mode;
    let server_address = args.server_address;
    let serverport_number = args.serverport_number;
    let client_address = args.client_address;
    let loops = args.loops;

    let serverport: Port = Port::try_from(serverport_number).unwrap();
    let localaddress = Ipv4Endpoint::new(server_address, serverport);

    match mode.as_ref()
    {
        "server" =>{
            server(localaddress, initialize_dpdk_fn(server_address), loops); 
        }
        "client" => {
            client(localaddress, initialize_dpdk_fn(client_address), loops);
        }
        _ => {
            panic!("please select a mode: client or server")
        }
    }

    Ok(())
}


//get the server address from commandline argument
fn server(localaddress: Ipv4Endpoint,mut libos:LibOS<DPDKRuntime>, loops: u8)
{
    let socket_fd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
    libos.bind(socket_fd, localaddress).unwrap();
    libos.listen(socket_fd, 1024).unwrap();
    let mut start = std::time::Instant::now();
    let mut end = std::time::Instant::now();
    let mut timeloggingdiff = std::time::Duration::new(0, 0);
    let mut nbytes: usize = 0;



    //Instead of sockets with read and write we use
    //Queues with push and pop - queue
    let mut queue: Vec<u64> = Vec::with_capacity(1_000);


    let qt_accept = libos.accept(socket_fd).unwrap();
    queue.push(qt_accept);

    for _ in 0..loops {
        //all the operations are asynchronous by default
        //wait returns a qtoken and blocks on completion
        let (i, fd, result) = libos.wait_any2(&queue);
        //Native zero copy
        queue.swap_remove(i);

        //result is the operation of the qtoken that just finished executing 
    
        match result {
            OperationResult::Accept(fd) => {
                let q = libos.pop(fd).unwrap();
                queue.push(q);
            }

            OperationResult::Pop(_, buf) => {
                //echo this back

                nbytes += buf.len();
                let buf2 = buf.clone();
                start = std::time::Instant::now();
                let q = libos.push2(fd, buf2).unwrap();
                queue.push(q);
            }

            OperationResult::Push => {

                end = std::time::Instant::now();
                timeloggingdiff = end-start;
                println!("time difference: {:?}", timeloggingdiff);
                let q = libos.pop(fd).unwrap();
                queue.push(q);
            }
            _ => {
                panic!("wrong oper found: {:?}", i);
            }
        }
    }
}

fn client(localaddress: Ipv4Endpoint, mut libos:LibOS<DPDKRuntime>, loops: u8)
{
    let socket_fd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
    //let mut queue: Vec<u64> = Vec::with_capacity(1_000);
    let mut q = libos.connect(socket_fd, localaddress).unwrap();
    //queue.push(qt_connect);

    let mut start = std::time::Instant::now();
    let mut end = std::time::Instant::now();
    let mut timeloggingdiff = std::time::Duration::new(0, 0);


    let mut x = 123;
    for _ in 0..loops {
        //wait blockson I/O operations and returns the result 
        let (fd, result) = libos.wait2(q);
        //queue.swap_remove(0);

        //push, pop are ansyc - return a qtoken q for 
        //blocking on I/O completion 
        match result {
            OperationResult::Connect => {
                q = libos.push2(fd, makepkt(&libos, 8,123)).unwrap();
                //queue.push(q);
            }

            OperationResult::Push => {
                start = std::time::Instant::now();
                q = libos.pop(fd).unwrap();
                //queue.push(q);
                x=x+1;
            }
            OperationResult::Pop(_, _) => {

                end = std::time::Instant::now();
                timeloggingdiff = end-start;
                
                println!("time difference: {:?}", timeloggingdiff);
                q = libos.push2(fd, makepkt(&libos,8, x)).unwrap();
            }
            _ => {
                panic!("wrong oper found: {:?}", result);
            }
        }
    }
}

//setting up DPDK Runtime
fn initialize_dpdk_fn(address:Ipv4Addr) -> LibOS<DPDKRuntime>
{
    //arguments required for DPDK runtime
    let strings = vec!["--proc-type=auto", "--vdev=net_vdev_netvsc0,iface=eth1"];
    let vector_cstring: Vec<CString> = strings
        .into_iter()
        .map(|s| CString::new(s).expect("Error creating CString"))
        .collect();

    //need rt for LibOS struct
    let rt: DPDKRuntime = initialize_dpdk(
        address,
        &vector_cstring,
        HashMap::new(),
        false,
        false,
        1500,
        536,
        false,
        false,
    )
.unwrap();

let libos = LibOS::new(rt).unwrap();

libos

}

//change this to accomodate any packet size not just 64 bits 
fn makepkt(libos: &LibOS<DPDKRuntime>,reply_size: usize, i: usize) -> DPDKBuf {

    let mut pktbuf = libos.rt().alloc_body_mbuf();
    // Factory packet.
    let stamp_slice = i.to_ne_bytes();
    let pktbuf_slice = unsafe { pktbuf.slice_mut() };
    for i in 0..stamp_slice.len() {
        pktbuf_slice[i] = stamp_slice[i];
    }
    drop(pktbuf_slice);
    pktbuf.trim(pktbuf.len() - reply_size);

    DPDKBuf::Managed(pktbuf)
}


// fn handle_connection(mut stream: TcpStream) {

//     let (status_line, filename) = if buffer.starts_with(get) {
//         ("HTTP/1.1 200 OK", "hello.html")
//     } else {
//         ("HTTP/1.1 404 NOT FOUND", "404.html")
//     };

//     let contents = fs::read_to_string(filename).unwrap();

//     let response = format!(
//         "{}\r\nContent-Length: {}\r\n\r\n{}",
//         status_line,
//         contents.len(),
//         contents
//     );

//     stream.write(response.as_bytes()).unwrap();
//     stream.flush().unwrap();
// }