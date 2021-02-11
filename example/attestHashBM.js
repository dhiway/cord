// Required imports
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { Keyring } = require('@polkadot/keyring');
//const { deriveAddress, importPrivateKey } = require('@substrate/txwrapper');
const {  mnemonicToMiniSecret, mnemonicGenerate, cryptoWaitReady, encodeAddress, xxhashAsHex, blake2AsHex } = require('@polkadot/util-crypto') ;
const { compactAddLength, stringToU8a } = require('@polkadot/util');

const customTypes = require('../custom-types.json');

async function main () {
    await cryptoWaitReady();

    // Initialise the provider to connect to the local node
    //const provider = new WsProvider('wss://cord.dway.io');
    const provider = new WsProvider('ws://localhost:9944');
    const api = await ApiPromise.create({ provider, types: customTypes});

    const keyring = new Keyring({ type: 'sr25519' })

    /* Generate any new key */
    //const new_phrase = mnemonicGenerate();
    //console.log("New Key Phrase: ", new_phrase);
    //const new_seed = mnemonicToMiniSecret(new_phrase);
    //const new_key = keyring.addFromSeed(new_seed)

    // Retrieve the last timestamp (to check API is working)
    const now = await api.query.timestamp.now();
    const new_key = keyring.addFromUri('//Eve');
    //const rootkey = keyring.addFromUri(process.env.STASH_URI);

    var i;
    const ctype = "{ name, company }" + now;
    const chash = xxhashAsHex(ctype, 256);
    var apiBeforeSign = api.tx.mtype.anchor(chash);
    const nonce = await api.rpc.system.accountNextIndex(new_key.address);
    console.log("timestamp & nonce:  ", now, nonce);
    var txHash = await apiBeforeSign.signAndSend(new_key, {nonce: -1}, (blk) => {
	// status would still be set, but in the case of error we can shortcut
	// to just check it (so an error would indicate InBlock or Finalized)
	if (blk.dispatchError) {
	    if (blk.dispatchError.isModule) {
		// for module errors, we have the section indexed, lookup
		const decoded = api.registry.findMetaError(blk.dispatchError.asModule);
		const { documentation, name, section } = decoded;
		
		console.log(`${section}.${name}: ${documentation.join(' ')}`);
	    } else {
		// Other, CannotLookup, BadOrigin, no extra info
		console.log(blk.dispatchError.toString());
	    }
	}
	if (blk.status.isInBlock) {
	    console.log(`Written to block: ${blk.status.asInBlock}`);
	    var attest;
	    var link;
	    var mhash;
	    var attests = [];
	    for (i = 0; i < 10000; i++) {
		link = "https://dhiway.com/"+ now +"/" + i;
		console.log(link);
		mhash = xxhashAsHex(link, 256);
		attest = api.tx.mark.anchor(mhash, chash, undefined);
		attests.push(attest);
	    }
	    api.tx.utility.batch(attests).signAndSend(new_key, {nonce: -1}, (blk) => {
		if (blk.status.isInBlock) {
		    console.log(`Written to block: ${blk.status.asInBlock}`);
		}
	    })
	    
	    console.log("DONE");
	}
    });
}


main().catch(console.error).finally(async () => {
    /* Give time to finalize */
    await new Promise(resolve => setTimeout(resolve, 30000));
    process.exit();
});
