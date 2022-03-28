use catnip::protocols::ipv4::Ipv4Endpoint;
use std::{ffi::CString, net::Ipv4Addr, env,collections::HashMap};
use demikernel::catnip::{dpdk::initialize_dpdk,runtime::DPDKRuntime,memory::DPDKBuf};
use catnip::{libos::LibOS, operations::OperationResult, protocols::ip::Port};


fn main()
{ 
    let args: Vec<String> = env::args().collect();
    let mode: &str = &args[1];
    println!("THIS IS THE SERVER");

    let server_address = Ipv4Addr::new(10, 4, 0, 7);
    let serverport:Port = Port::try_from(12345).unwrap();
    let localaddress = Ipv4Endpoint::new(server_address, serverport);
    let client_address = Ipv4Addr::new(10, 4, 0, 5);

    println!("Mode = {}", mode);
    println!("at IP address: {}", server_address);

    let strings = vec!["--proc-type=auto", "--vdev=net_vdev_netvsc0,iface=eth1"];
    let vector_cstring: Vec<CString> = strings.into_iter().map(|s| {CString::new(s).expect("Error creating CString")}).collect();

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
    ).unwrap();


    //need to call the LibOS constructor
    let mut libos = LibOS::new(rt).unwrap();

    match mode
    {        
        "server" => 
        {
            let socket_fd    = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();       
            libos.bind(socket_fd,localaddress).unwrap();
            libos.listen(socket_fd, 1024).unwrap();
            let qt_accept    = libos.accept(socket_fd).unwrap();

            match libos.wait2(qt_accept) 
            {
                (qd2, OperationResult::Accept(qt_accept))=> 
                {
                    println!("Connection accepted at server side");
                    let q = libos.pop(qt_accept).unwrap();
                    let (_, oper) = libos.wait2(q);

                    
                    //if pop then push it back 
                    if let OperationResult::Pop(_,buf) = oper { 
                        println!("popping");
                        //println!("{:?}",buf);
                        //push back
                        let q = libos.push2(qt_accept,makepkt(&libos, 10)).unwrap();
                        let (_, oper) = libos.wait2(q);
                        if let OperationResult:: Push = oper    { 
                            println!("pushing");
                            println!("{:?}",oper);
                        }
           
                       else {
                        println!("{:?}",oper);
                           panic!("Push error");
                        }

                     }
                    else {
                        panic!("pop error");

                    }
                }

                _ =>
               {
               }
            }
            
        }

        "client" =>
            {
            /*----------------------------------------------------------
            ||                        CLIENT                          ||
            ----------------------------------------------------------*/
            println!("CLIENT!");
            let socket_fd    = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
            let qt_connect   = libos.connect(socket_fd, localaddress).unwrap();
            let (qd2, oper2) = libos.wait2(qt_connect) ;

            match oper2 
            {
                OperationResult::Connect => 
                {
                    println!("connected");
                    let q = libos.push2(qd2,makepkt(&libos, 10)).unwrap();
                    let (_, oper) = libos.wait2(q);
                    if let OperationResult:: Push  = oper    { 
                    }
       
                   else {panic!("Push error");}
                }

                _ =>
                {
                    panic!("Wrong oper found?")
                }
               
            }

    
        }
            _=> 
                {
                    panic!("please select a mode: client or server")
                }
    
        }
        }
    
    fn makepkt(libos: &LibOS<DPDKRuntime>, i: u32) -> DPDKBuf {

        let mut pktbuf = libos.rt().alloc_body_mbuf();

        // Factory packet.
        let stamp_slice = i.to_ne_bytes();
        let pktbuf_slice = unsafe { pktbuf.slice_mut() };
        for i in 0..stamp_slice.len() {
            pktbuf_slice[i] = stamp_slice[i];
        }
        drop(pktbuf_slice);
        pktbuf.trim(pktbuf.len() - 4);

        DPDKBuf::Managed(pktbuf)
    }
