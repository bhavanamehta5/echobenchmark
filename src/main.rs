//echo benchmark for performance testing
use std::{collections::HashMap, ffi::CString, net::Ipv4Addr};
use catnip::{libos::LibOS, operations::OperationResult, protocols::ip::Port, protocols::ipv4::Ipv4Endpoint};
use demikernel::catnip::{dpdk::initialize_dpdk, memory::DPDKBuf, runtime::DPDKRuntime};
use anyhow::{Error, Result};
use clap::Parser;

//Parsing mode, server IPv4 address, server port and client IPv4 address
//via command line using clap 

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
    port: u16,
    
    /// client address
    #[clap(short, long, default_value_t = Ipv4Addr::new(10, 4, 0, 5))]
    client_address: Ipv4Addr,
}

fn main() -> Result<(), Error>  
{
    let args = Args::parse();
    let serverport_number = args.port;
    let mode = args.mode;
    let server_address = args.server_address;
    let serverport_number = args.port;
    let client_address = args.client_address;

    let serverport: Port = Port::try_from(serverport_number).unwrap();
    let localaddress = Ipv4Endpoint::new(server_address, serverport);

    //calling .deref() so that String gets automagically turned into &str 
    //for comparisons with literals.
    match mode.as_deref()
    {
        //initialize_dpdk_fn gives you Libos with DPDK Runtime
        "server" =>{
            server(localaddress, initialize_dpdk_fn(server_address)); 
        }

        "client" => {
            client(localaddress, initialize_dpdk_fn(client_address));
        }

        _ => {
            panic!("please select a mode: client or server")
        }
    }

    Ok(())
}


fn server(localaddress: Ipv4Endpoint,mut libos:LibOS<DPDKRuntime>)
{
    let mut start = std::time::Instant::now();
    let mut end = std::time::Instant::now();
    let mut timeloggingdiff = std::time::Duration::new(0, 0);
    let mut nbytes: usize = 0;

    let socket_fd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
    libos.bind(socket_fd, localaddress).unwrap();
    libos.listen(socket_fd, 1024).unwrap();

    //Instead of sockets with read and write we use
    //Queues with push and pop - queue
    let mut queue: Vec<u64> = Vec::with_capacity(1_0000);
    //first queue token in the vector queue
    let qt_accept = libos.accept(socket_fd).unwrap();
    queue.push(qt_accept);

    //wait on all the qtokens generates
    //note: wait_any2 goes through all pushes first and then 
    //goes and checks for pop qtokens. Push is always high priotity
    loop {
        //all the operations are asynchronous by default
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
                start = std::time::Instant::now();
                nbytes += buf.len();
                let buf2 = buf.clone();
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

fn client(localaddress: Ipv4Endpoint, mut libos:LibOS<DPDKRuntime>)
{
    let socket_fd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
    let mut queue: Vec<u64> = Vec::with_capacity(1_0000);
    let mut q = libos.connect(socket_fd, localaddress).unwrap();
    queue.push(q);

    let mut start = std::time::Instant::now();
    let mut end = std::time::Instant::now();
    let mut timeloggingdiff = std::time::Duration::new(0, 0);


    loop
    {
        let (i, fd, result) = libos.wait_any2(&queue);
        queue.swap_remove(i);

        //push, pop are ansyc - return a qtoken q for 
        //blocking on I/O completion 
        match result {
            OperationResult::Connect => {
                q = libos.push2(fd, makepkt(&libos, 8,1)).unwrap();
                queue.push(q);
            }

            //Open loop main logic:
            //This enables no waiting for pop, multiple pushes can happen without waiting for pop to finish
            //after every pop there is a push
            //there is a pop for every push so whatever data is pushed will have a way to come back and be popped out
            //wait on pushes is very less since they happen instantaneously
            //wait always goes to pop once it checks for all push qtokens in the queue
            OperationResult::Push => {
                start = std::time::Instant::now();

                let q2 = libos.pop(fd).unwrap();
                queue.push(q2);

                let q3 = libos.push2(fd, makepkt(&libos, 8,1)).unwrap();
                queue.push(q3);
            }

            //since we already have a pop for every push we leave this empty
            OperationResult::Pop(_, _) => {
                end = std::time::Instant::now();
                timeloggingdiff = end-start;
                println!("time difference: {:?}", timeloggingdiff);

            }
            _ => {
                panic!("wrong oper found: {:?}", result);
            }
        }
    }
}

//initializing DPDK Runtime, libos construction
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

//accomodates packet of any size 
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