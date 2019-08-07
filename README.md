# Cash:web Keyserver (Rust Implementation)
[![Build Status](https://travis-ci.org/hlb8122/keyserver-rust.svg?branch=master)](https://travis-ci.org/hlb8122/keyserver-rust)

[**Golang Implementation**](https://github.com/cashweb/keyserver/)

This repository hosts a reference implementation of the Cash:web Keyserver protocol. The goal is to provide a distributed, simple-to-use and cryptographically verifiable way to look up xpubkeys, and other metadata, from their hashes. The hashes are commonly available within Bitcoin Cash Addresses such as *bitcoincash:pqkh9ahfj069qv8l6eysyufazpe4fdjq3u4hna323j*. This enables wallets to query a network of metadata nodes to find out various information for contacting or paying the owners in a secure and private manner. Additionally, wallets managing a key can advertise special capabilities they support.

## Why not existing systems?

Traditional keyservers are subject to certificate spamming attacks. By being a first-class citizen in the cryptocurrency ecosystem, we are able to charge for key updates. This prevents an explosion of advertised certificates, and provides some funding for node operators. Other systems like OpenAlias, require that you trust the service provider is providing the correct addresses, while this keyserver cannot forge such updates as they are tied to a keyid which has been provided via another channel. At most, a malicious keyserver can censor a particular key, in which case other keyservers will provide it.
