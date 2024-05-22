# QR Code Scanner wasm

A simple QR code scanner using WebAssembly.

## Creating the certificates for SSL
Note that at this time we are only aware of a procedure for MacOS.

To create the certificates for SSL, you can use the following command:
```bash
brew install mkcert
```
After that, we want to also add `nss` since we want to test stuff on Firefox:
```bash
brew install nss
```
Then we can run the following command to create the certificates:
```bash
mkcert -install
```

Now, we can create the certificates for the platform:
```bash
mkcert -cert-file nginx/${DOMAIN}.pem -key-file nginx/${DOMAIN}-key.pem ${DOMAIN}
```
and write to your `.env` file domain name you want to use.

## Running the server
To run the server, you can use the following command:
```bash
trunk serve --address 0.0.0.0 --port 9898 --tls-key-path ./nginx/${DOMAIN}-key.pem --tls-cert-path ./nginx/${DOMAIN}.pem
```

You can now access the server on `https://${DOMAIN}:9898`.

