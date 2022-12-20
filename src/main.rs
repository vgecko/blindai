// Copyright 2022 Mithril Security. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![forbid(unsafe_code)]

use identity::MyIdentity;
use std::sync::Arc;
use std::thread;
mod identity;
mod model;
mod model_store;
use crate::client_communication::Exchanger;
use anyhow::Result;
use model_store::ModelStore;
use rouille::Request;
mod client_communication;
use log::{debug, error, info, log_enabled, Level};
use ureq::Error::Status;
use ureq::OrAnyStatus;

// ra
use env_logger::Env;
use ring::{digest, digest::Digest};
use sgx_isa::{Report, Targetinfo};
use std::io::prelude::*;
use std::time;
use std::time::Duration;

struct EnclaveProperties {
    quote: Vec<u8>,
    collateral: Vec<u8>,
}

impl<'a> EnclaveProperties {
    fn init(quote: Vec<u8>, collateral: Vec<u8>) -> Self {
        Self { quote, collateral }
    }

    fn get_quote(&'a self) -> &'a [u8] {
        &self.quote[..]
    }
    fn get_collateral(&'a self) -> &'a [u8] {
        &self.collateral[..]
    }
}

fn main() -> Result<()> {
    // Make debugging easier by enabling rust backtrace inside enclave
    std::env::set_var("RUST_BACKTRACE", "full");
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();

    let (certificate, signing_key_seed) = identity::create_certificate().unwrap();
    let my_identity = MyIdentity::from_cert(certificate, signing_key_seed);
    let enclave_identity = my_identity.tls_identity.clone();

    let exchanger_temp = Arc::new(Exchanger::new(
        Arc::new(ModelStore::new()),
        Arc::new(my_identity),
        1000000000,
        1000000,
    ));

    // Remote attestation
    // Connecting to the runner

    // Enclave held data hash
    let enclave_held_data = enclave_identity.cert_der.clone();
    let mut report_binding = digest::digest(&digest::SHA256, &enclave_identity.cert_der.clone());
    let report_data_slice: &[u8] = report_binding.as_ref();
    let mut report_data: Vec<u8> = vec![0; 32];
    report_data.extend_from_slice(report_data_slice);
    let mut reportdata: [u8; 64] = report_data.try_into().unwrap();

    let get_ti = ureq::get("http://127.0.0.1:11000/target-info")
        .call()
        .or_any_status()
        .unwrap();
    let len: usize = get_ti.header("Content-Length").unwrap().parse().unwrap();
    let mut bytes: Vec<u8> = Vec::with_capacity(len);
    get_ti
        .into_reader()
        .take(10_000_000)
        .read_to_end(&mut bytes)
        .unwrap();
    debug!("target info is {:?} ", bytes);

    let sliced_data: &[u8] = &bytes;
    // retrieving targeinfo
    let targetinfo = Targetinfo::try_copy_from(sliced_data).unwrap();
    debug!(
        "Attestation : the targetinfo generated is : {:?}",
        targetinfo
    );
    let report = Report::for_target(&targetinfo, &reportdata);
    debug!("Attestation : the report generated is : {:?}", report);
    let send_report = ureq::post("http://127.0.0.1:11000/report").send_bytes(report.as_ref());

    let get_quote = ureq::get("http://127.0.0.1:11000/get-quote")
        .call()
        .or_any_status()
        .unwrap();
    let len_quote: usize = get_quote.header("Content-Length").unwrap().parse().unwrap();
    let mut bytes_quote: Vec<u8> = Vec::with_capacity(len_quote);
    get_quote
        .into_reader()
        .take(10_000_000)
        .read_to_end(&mut bytes_quote)
        .unwrap();
    debug!("Attestation : Quote is {:?} ", bytes_quote);

    let get_collateral = ureq::get("http://127.0.0.1:11000/getcollateral")
        .call()
        .or_any_status()
        .unwrap();
    let collateral = get_collateral.into_string().unwrap();
    debug!("Attestation : Collateral is {:?} ", collateral.clone());

    let untrusted_server = rouille::Server::new_ssl(
        "0.0.0.0:9923",
        {
            let _enclave_identity_cloned = enclave_identity.clone();
            move |request: &Request| {
                println!("Requested enclave TLS certificate");
                rouille::Response::from_data("application/octet-stream", *b"")
                // TODO: Yassine doit mettre sa version de l'untrusted serveur adapté à rouille
                // let untrusted_server = untrusted_server.clone();
                // let trusted_cert = enclave_identity.cert_der.clone(); //enclave_held_data
                // let quote = bytes_quote.clone();
                // let collateral_data = collateral.clone();
                // let enclave_held_data_cert = enclave_held_data.clone();
                // untrusted_handles.push(thread::spawn(move || {
                //     for rq in untrusted_server.incoming_requests() {
                //         match rq.url() {
                //             "/" => {
                //                 if rq.method() == &tiny_http::Method::Get {
                //                     debug!("Attestation : Resquesting trusted cert");
                //                     rq.respond(Response::from_data(trusted_cert.clone()))
                //                         .unwrap();
                //                 }
                //             }

                //             "/quote" => {
                //                 if rq.method() == &tiny_http::Method::Get {
                //                     let quote_slice = quote.as_slice();
                //                     debug!("Attestation : Sending quote to client....");
                //                     rq.respond(Response::from_data(quote_slice)).unwrap();
                //                 }
                //             }

                //             "/collateral" => {
                //                 if rq.method() == &tiny_http::Method::Get {
                //                     let collateral_string = collateral_data.clone();
                //                     debug!("Attestation : Sending collateral to client....");
                //                     rq.respond(Response::from_string(collateral_string))
                //                         .unwrap();
                //                 }
                //             }
                //             "/enclave-held-data" => {
                //                 if rq.method() == &tiny_http::Method::Get {
                //                     let enclave_held_data_slice = enclave_held_data_cert.as_slice();
                //                     debug!("Attestation : Sending enclave_held_data to client....");
                //                     rq.respond(Response::from_data(enclave_held_data_slice))
                //                         .unwrap();
                //                 }
                //             }
                //              _ => panic!("need to return an error")
                //         };
                //     }
            }
        },
        include_bytes!("../host_server.pem").to_vec(),
        include_bytes!("../host_server.key").to_vec(),
    )
    .expect("Failed to start untrusted server")
    .pool_size(4);
    let (_untrusted_handle, _untrusted_sender) = untrusted_server.stoppable();

    thread::spawn(move || {
        let trusted_server = rouille::Server::new_ssl(
            "0.0.0.0:9924",
            move |request| {
                rouille::router!(request,
                    (POST) (/upload) => {
                        let reply = exchanger_temp.send_model(request);
                        exchanger_temp.respond(request, reply)
                    },

                    (POST) (/run) => {
                        let reply = exchanger_temp.run_model(request);
                        exchanger_temp.respond(request, reply)
                    },

                    (POST) (/delete) => {
                        let reply = exchanger_temp.delete_model(request);
                        exchanger_temp.respond(request, reply)
                    },

                    _ => rouille::Response::empty_404()
                )
            },
            enclave_identity.cert_der,
            enclave_identity.private_key_der,
        )
        .expect("Failed to start trusted server");
        let (_trusted_handle, _trusted_sender) = trusted_server.stoppable();
        _trusted_handle.join().unwrap();
    });
    println!("Now listening on port 9923 and 9924");
    _untrusted_handle.join().unwrap();

    Ok(())
}
