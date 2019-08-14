import requests
import bitcoin
from decimal import Decimal
import os
from bitcoin.core.key import CECKey
from bitcoin.wallet import P2PKHBitcoinAddress
from bitcoin.core import CMutableTransaction, CMutableTxIn
from addressmetadata_pb2 import *
from paymentrequest_pb2 import *
from bitcoinrpc.authproxy import AuthServiceProxy, JSONRPCException
from time import time
from hashlib import sha256

# Generate protobuf code
# protoc -I=./src/proto --python_out=./example ./src/proto/*.proto

# Run bitcoind in regtest mode
# bitcoind -daemon -regtest -zmqpubrawtx=tcp://127.0.0.1:28332 -rpcallowip=0.0.0.0/0 -server  \
# -rpcuser=username -rpcpassword=password -rpcport=18332
# bitcoin-cli -regtest -rpcuser=username -rpcpassword=password generate 101

BASE_URL = "http://0.0.0.0:8080"
bitcoin.SelectParams("regtest")

# Init bitcoin RPC
rpc_user = "username"
rpc_password = "password"
rpc_connection = AuthServiceProxy(
    "http://%s:%s@127.0.0.1:18332" % (rpc_user, rpc_password))

# Generate keys
secret = os.urandom(16)
keypair = CECKey()
keypair.set_secretbytes(secret)
private_key = keypair.get_privkey()
public_key = keypair.get_pubkey()

# Generate key addr
key_addr = str(P2PKHBitcoinAddress.from_pubkey(public_key))

# Put key without payment
response = requests.put(url=BASE_URL + "/keys/" + key_addr)
assert(response.status_code == 402) # Payment required

# Construct payment
payment_request = PaymentRequest.FromString(response.content)
payment_details_raw = payment_request.serialized_payment_details
payment_details = PaymentDetails.FromString(payment_details_raw)

# Create inputs
# Convert Satoshi to Bitcoin
price = Decimal(payment_details.outputs[0].amount) / 1_00_000_000
fee = Decimal(5) / 10_000_000
utxos = rpc_connection.listunspent()
inputs = []
input_value = Decimal(0)
for utxo in utxos:
    if input_value < price + fee:
        inputs.append({
            "txid": utxo["txid"],
            "vout": utxo["vout"]
        })
        input_value += utxo["amount"]
    else:
        break

# Create outputs
my_addr = utxo["address"]
change = input_value - price - fee
p2pkh = payment_details.outputs[0].script
payment_addr = str(P2PKHBitcoinAddress.from_scriptPubKey(p2pkh))
outputs = [
    {
        payment_addr: price  # Payment output
    },
    {
        my_addr: change  # Change output
    }
]

# Create tx
raw_tx_unsigned = rpc_connection.createrawtransaction(inputs, outputs)
signed_raw_tx = bytes.fromhex(
    rpc_connection.signrawtransactionwithwallet(raw_tx_unsigned)["hex"])

# Construct payment message
payment = Payment(merchant_data=payment_details.merchant_data,
                  transactions=[signed_raw_tx])
payment_raw = payment.SerializeToString()

# Send payment
payment_url = BASE_URL + payment_details.payment_url
headers = {
    "accept": "application/bitcoin-payment",
    "content-type": "application/bitcoin-paymentack"
}
response = requests.post(url=payment_url, data=payment_raw,
                         headers=headers, allow_redirects=False)
payment_ack = PaymentACK.FromString(response.content)
print("PaymentACK memo:", payment_ack.memo)
token_url = response.headers["Location"]

# Construct Payload
header = Header(Name="Something wicked", Value="this way comes")
metadata_field = MetadataField(
    Headers=[header], Metadata=b'This gonna be so fucking fast')
timestamp = int(time())
payload = Payload(Timestamp=timestamp, Rows=[metadata_field])

# Sign
raw_payload = payload.SerializeToString()
h = sha256()
h.update(raw_payload)
digest = h.digest()
signature, _ = keypair.sign_compact(digest)

# Address metadata
addr_metadata = AddressMetadata(
    PubKey=public_key, Payload=payload, Type=1, Signature=signature)
raw_addr_meta = addr_metadata.SerializeToString()

# Put metadata using payment token
response = requests.put(url=token_url, data=raw_addr_meta)

# Get metadata
response = requests.get(url=BASE_URL + "/keys/" + key_addr)
addr_metadata = AddressMetadata.FromString(response.content)
print(addr_metadata)
