//echo benchmark for performance testing
use std::{collections::HashMap, env, ffi::CString, net::Ipv4Addr};
use catnip::{libos::LibOS, operations::OperationResult, protocols::ip::Port, protocols::ipv4::Ipv4Endpoint};
use demikernel::catnip::{dpdk::initialize_dpdk, memory::DPDKBuf, runtime::DPDKRuntime};

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode: &str = &args[1];
    let mut start = std::time::Instant::now();
    let mut end = std::time::Instant::now();
    let mut timeloggingdiff = std::time::Duration::new(0, 0);
    let mut nbytes: usize = 0;

    match mode {
        "server" => {

            //get the server address from commandline argument
            let server_address = if args.len() > 2 {
                match args[2].parse() {
                    Ok(ip) => {
                        println!("Using user-provided local address: {:?}", ip);
                        ip
                    },
                    Err(_) => panic!("Cannot parse IP address given as command-line argument.")
                }
            } else {
                Ipv4Addr::new(10, 4, 0, 7)
            };

            //get port number from commandline argument
            let serverport_number = if args.len() > 3 {
                match args[3].parse() {
                    Ok(port) => {
                        println!("Using user-provided local address: {:?}", port);
                        port
                    },
                    Err(_) => panic!("Cannot parse IP address given as command-line argument.")
                }
            } else {
                12345
            };

            let serverport: Port = Port::try_from(serverport_number).unwrap();
            let localaddress = Ipv4Endpoint::new(server_address, serverport);

            //number of times client send data to the server
            let loops = if args.len() > 4 {
                match args[4].parse() {
                    Ok(x) => {
                        println!("Looping {:?} number of times", x);
                        x
                    },
                    Err(_) => panic!("Cannot parse number of loop as command-line argument.")
                }
            } else {
                20
            };
    
            println!("Mode = {}", mode);
            println!("at IP address: {}", server_address);
        
            let strings = vec!["--proc-type=auto", "--vdev=net_vdev_netvsc0,iface=eth1"];
            let vector_cstring: Vec<CString> = strings
                .into_iter()
                .map(|s| CString::new(s).expect("Error creating CString"))
                .collect();
        
            let rt: DPDKRuntime = initialize_dpdk(
                server_address,
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
            let mut libos = LibOS::new(rt).unwrap();

            println!("THIS IS THE SERVER");

            let socket_fd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
            libos.bind(socket_fd, localaddress).unwrap();
            libos.listen(socket_fd, 1024).unwrap();

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
                        println!("Connection accepted at server side");

                        start = std::time::Instant::now();
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

        "client" => {

    //get the client address from commandline argument
    let client_address = if args.len() > 2 {
        match args[2].parse() {
            Ok(ip) => {
                println!("Using user-provided local address: {:?}", ip);
                ip
            },
            Err(_) => panic!("Cannot parse IP address given as command-line argument.")
        }
    } else {
        Ipv4Addr::new(10, 4, 0, 5)
    };

    //get the server address from commandline argument
    let server_address = if args.len() > 2 {
        match args[3].parse() {
            Ok(ip) => {
                println!("Using user-provided local address: {:?}", ip);
                ip
            },
            Err(_) => panic!("Cannot parse IP address given as command-line argument.")
        }
    } else {
        Ipv4Addr::new(10, 4, 0, 7)
    };
    //get port number from commandline argument
    let serverport_number = if args.len() > 3 {
        match args[4].parse() {
            Ok(port) => {
                println!("Using user-provided local address: {:?}", port);
                port
            },
            Err(_) => panic!("Cannot parse IP address given as command-line argument.")
        }
    } else {
         12345
    };

    let serverport: Port = Port::try_from(serverport_number).unwrap();
    let localaddress = Ipv4Endpoint::new(server_address, serverport);
  
    //number of times client sends data to the server
    let loops = if args.len() > 4 {
        match args[5].parse() {
            Ok(x) => {
                println!("Looping {:?} number of times", x);
                x
            },
            Err(_) => panic!("Cannot parse number of loop as command-line argument.")
        }
    } else {
         20
    };

    println!("Mode = {}", mode);
    println!("at IP address: {}", client_address);

    //setting up DPDK Runtime

    //arguments required for DPDK runtime
    let strings = vec!["--proc-type=auto", "--vdev=net_vdev_netvsc0,iface=eth1"];
    let vector_cstring: Vec<CString> = strings
        .into_iter()
        .map(|s| CString::new(s).expect("Error creating CString"))
        .collect();

    //need rt for LibOS struct
    let rt: DPDKRuntime = initialize_dpdk(
        client_address,
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

    //calling LibOS constructor
    let mut libos = LibOS::new(rt).unwrap();

 
            /*----------------------------------------------------------
            ||                        CLIENT                          ||
            ----------------------------------------------------------*/
            let socket_fd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
            //let mut queue: Vec<u64> = Vec::with_capacity(1_000);
            let mut q = libos.connect(socket_fd, localaddress).unwrap();
            //queue.push(qt_connect);

            let mut x = 123;
            for _ in 0..20 {
                //wait blockson I/O operations and returns the result 
                let (fd, result) = libos.wait2(q);
                //queue.swap_remove(0);

                //push, pop are ansyc - return a qtoken q for 
                //blocking on I/O completion 
                match result {
                    OperationResult::Connect => {
                        println!("connectedSDEOJFOIWEJFIEWJF");

                        start = std::time::Instant::now();
                        q = libos.push2(fd, makepkt(&libos, 8,123)).unwrap();
                        //queue.push(q);
                    }

                    OperationResult::Push => {
                        start = std::time::Instant::now();
                        println!("push");

                        q = libos.pop(fd).unwrap();
                        //queue.push(q);
                        x=x+1;
                    }
                    OperationResult::Pop(_, _) => {

                        end = std::time::Instant::now();
                        println!("pop");

                        timeloggingdiff = end-start;
                        
                        println!("time difference: {:?}", timeloggingdiff);
                        q = libos.push2(fd, makepkt(&libos,8, x)).unwrap();
                        println!("{:}", x);
                    }
                    _ => {
                        panic!("wrong oper found: {:?}", result);
                    }
                }
            }
        }
        _ => {
            panic!("please select a mode: client or server")
        }
    }
}



//change this to accomodate any packet size not just 64 bits 
fn makepkt(libos: &LibOS<DPDKRuntime>,reply_size: usize, i: u64) -> DPDKBuf {

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

