## Keys for CORD

CORD is configured to work with admin generated keys for session and external user accounts. The default configuration has a few pre-configured accounts/keys which can be replaced at runtime.

## Default Keys

### User Accounts
Use the accounts below with a [wallet](https://polkadot.js.org/extension/) to connect and transact with CORD.


``` bash
Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//1//controller` is account:
  Secret seed:      0x2963cfa2f1d39befd4e0256c9a9be18db499ac4f72d6387f0280967db6234076
  Public key (hex): 0x4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49
  Account ID:       0x4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49
  SS58 Address:     5DWfCZZiszyGPHugjyUfyKWXbYuXFCkCUUYJJfiiL8SXUW1P

Secret phrase `replace power say exhibit portion parent badge artefact program cricket tape purity` is account:
  Secret seed:      0xedc737ae7be3537911015619dbb16460328cfcef1686d48a546b5070c785e163
  Public key (hex): 0x86703071df386fa21ce085228476b0656bda12c0f05f4749be65830d8eae4158
  Account ID:       0x86703071df386fa21ce085228476b0656bda12c0f05f4749be65830d8eae4158
  SS58 Address:     5F6ybobhyyQGdXKy76L1oboVeywzH79jSqDbeRD4kX5ffVAH

Secret phrase `cook ready tuna avoid wire device ordinary conduct unfair metal stick health` is account:
  Secret seed:      0xa3dc559e5ed9a75982adf6d9700505d942e00084d333819c06cb83419d414af4
  Public key (hex): 0x428879f73dab6604bc954c486a309bd5dc65a20c9c2ba911b34691fa2f60e733
  Account ID:       0x428879f73dab6604bc954c486a309bd5dc65a20c9c2ba911b34691fa2f60e733
  SS58 Address:     5DZwZQUShKreSzfA78EvCBYzpc5Hi5VGTNg47Toq87fLT5pC

Secret phrase `such husband flock early fringe clever disorder glow recall ramp alpha congress` is account:
  Secret seed:      0xeb9444c91c5167ae369e64171b9d31574908fe20dedb26eb41c9d868a6c99a6f
  Public key (hex): 0x181194d1b25a985a5b25c83ed3c22b3e5157ef6a34ac8789c809dd24cb7ab84b
  Account ID:       0x181194d1b25a985a5b25c83ed3c22b3e5157ef6a34ac8789c809dd24cb7ab84b
  SS58 Address:     5CcGEF5APCYQo4JwRd9yUWWFMDWToHHDgSKfYbFvAdJhvRun
  ```

## Session Keys
The keys are generated using the [prep_node_keys](scripts/prep_node_keys.sh) srcipt with the help of a secret. 

### Using the pre-configured accounts

The pre-configured accounts are hard derivatives of a master account. 
``` bash

Secret phrase `romance gold laptop describe cattle hedgehog like exist menu autumn pool cream` is account:
  Secret seed:      0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4
  Public key (hex): 0x7c1ab305b6706d7ebd19dd842ec95919f7af11a370564956df605eb3e4084616
  Account ID:       0x7c1ab305b6706d7ebd19dd842ec95919f7af11a370564956df605eb3e4084616
  SS58 Address:     5EsRj5EnDi3vDeMAnQ62hJHwRX641935rnTpzH1q41N95fws
  ```

 ### Aura and Grandpa Keys - Dev Node
``` bash
Account Details (
Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//1//controller` is account:
  Secret seed:      0x2963cfa2f1d39befd4e0256c9a9be18db499ac4f72d6387f0280967db6234076
  Public key (hex): 0x4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49
  Account ID:       0x4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49
  SS58 Address:     5DWfCZZiszyGPHugjyUfyKWXbYuXFCkCUUYJJfiiL8SXUW1P

Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//1//aura` is account:
  Secret seed:      0xc1f18f7682803015ad7417f6db6098857c629948449089b570f8744242f62989
  Public key (hex): 0xa047ce4974347c3972ae4a2292ba1fad2b4ece9d6eaffd5286c185985dc92401
  Account ID:       0xa047ce4974347c3972ae4a2292ba1fad2b4ece9d6eaffd5286c185985dc92401
  SS58 Address:     5FgrrDx5H3jx38Xe3mD883FPFdRmDyEEoNLA3nMbPd5uzt26

Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//1//grandpa` is account:
  Secret seed:      0x8addcbed10cb147f1f47e0a8b751fb825a249f6aab544ffb49301a955c151f5a
  Public key (hex): 0x03c22070787b00627c5817c34451e10a181f5c257dc44d49d0c6a9e46581ba27
  Account ID:       0x03c22070787b00627c5817c34451e10a181f5c257dc44d49d0c6a9e46581ba27
  SS58 Address:     5C9dfj4zZdFVg8ndFEXzKwiqd9FtYiracERZStwNEXefWZfv
),
```
### Chain Spec Entries - Dev Node
``` bash
(
//5DkEcWqE9CAj2Cusx1XSXhmLvGQQ8qChQnDJuTYMCukSjNWU
hex!["4a62568acc66d594cb24744a24968c21decb3d824ac3cde58561a30ea4e33026"].into(),
//5EWvkWim11zojxkiRX9SouCAq67jKC6HiGJGEBZrPEhqMXnc
hex!["6c784086bd9a33ca691e73f14ac0fd28a71a653f7d0bf3d1c8f06efc3bc88059"].unchecked_into(),
//5HczPzT6FTLYBD6rNB85TgZUpa5tupRUmkJSd15xQrtyCwba
hex!["f5ccb2eedfa6adb37bae3f28aa388c5e1e4e87b0684f77020e89879a2299fbf3"].unchecked_into(),
),
```
The aura and grandpa keys needs to be inserted to each nodes through the polka UI (Developer- RPC calls) or via `curl`. Choose "author" and "insertKey". The fields can be filled like this:

#### Aura Session Key
``` bash
keytype: aura
suri: 0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//1//aura
publicKey: 0xa047ce4974347c3972ae4a2292ba1fad2b4ece9d6eaffd5286c185985dc92401
```
#### Grandpa Session Key
``` bash
keytype: gran
suri: 0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//1//grandpa
publicKey: 0x03c22070787b00627c5817c34451e10a181f5c257dc44d49d0c6a9e46581ba27
```

## Creating New Keys

``` bash
subkey generate --scheme sr25519   

Secret phrase `romance gold laptop describe cattle hedgehog like exist menu autumn pool cream` is account:
  Secret seed:      0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4
  Public key (hex): 0x7c1ab305b6706d7ebd19dd842ec95919f7af11a370564956df605eb3e4084616
  Account ID:       0x7c1ab305b6706d7ebd19dd842ec95919f7af11a370564956df605eb3e4084616
  SS58 Address:     5EsRj5EnDi3vDeMAnQ62hJHwRX641935rnTpzH1q41N95fws
```

  ### Secret
Use the "Secret seed" entry from the new account as the root to generate derived accounts

``` bash
0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4
```
#### Export the secret key

```bash
export SECRET="0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4"
```
Run the [Script](scripts/prep_node_keys.sh) with number of validators
``` bash 
./prep_node_keys.sh <no. of nodes>

eg: ./prep_node_keys.sh 3 //for testnet
    ./prep_node_keys.sh 1 //for local dev node
```

## Test Net Configuration

Root keys and User accounts are same as the dev node. 

### Session Keys
``` bash
Account Details (
Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//1//controller` is account:
  Secret seed:      0x2963cfa2f1d39befd4e0256c9a9be18db499ac4f72d6387f0280967db6234076
  Public key (hex): 0x4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49
  Account ID:       0x4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49
  SS58 Address:     5DWfCZZiszyGPHugjyUfyKWXbYuXFCkCUUYJJfiiL8SXUW1P

Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//1//aura` is account:
  Secret seed:      0xc1f18f7682803015ad7417f6db6098857c629948449089b570f8744242f62989
  Public key (hex): 0xa047ce4974347c3972ae4a2292ba1fad2b4ece9d6eaffd5286c185985dc92401
  Account ID:       0xa047ce4974347c3972ae4a2292ba1fad2b4ece9d6eaffd5286c185985dc92401
  SS58 Address:     5FgrrDx5H3jx38Xe3mD883FPFdRmDyEEoNLA3nMbPd5uzt26

Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//1//grandpa` is account:
  Secret seed:      0x8addcbed10cb147f1f47e0a8b751fb825a249f6aab544ffb49301a955c151f5a
  Public key (hex): 0x03c22070787b00627c5817c34451e10a181f5c257dc44d49d0c6a9e46581ba27
  Account ID:       0x03c22070787b00627c5817c34451e10a181f5c257dc44d49d0c6a9e46581ba27
  SS58 Address:     5C9dfj4zZdFVg8ndFEXzKwiqd9FtYiracERZStwNEXefWZfv
),

Account Details (
Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//2//controller` is account:
  Secret seed:      0x53f040fae4411d0ad513528abe1ef29c117c396331bc112a13670fc3f2ee5643
  Public key (hex): 0x485494ec577ef5807683a52144343112af183ae54b95e349f0a4b122d3428014
  Account ID:       0x485494ec577ef5807683a52144343112af183ae54b95e349f0a4b122d3428014
  SS58 Address:     5DhYRwjpfvxYnBVD2QbPWrK5s9aJeji33nVNC99jQgwGSWpM

Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//2//aura` is account:
  Secret seed:      0xd1cc5ec58fe875a7a6cdda422acc3720708ff01bee5b65e6a371a36205591034
  Public key (hex): 0xd04e815f6b73d22e5d5948e7d05bab6c8cda3cb10e37660582fa918c61f4cc7b
  Account ID:       0xd04e815f6b73d22e5d5948e7d05bab6c8cda3cb10e37660582fa918c61f4cc7b
  SS58 Address:     5Gmq8xxxypVMA5YUjZQvsKFw71GyucC3ZnGrLTQ35C6ifcvQ

Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//2//grandpa` is account:
  Secret seed:      0xa757209bd76ca1b6cb70fad2bcf7a29be60ff704d0b20c86a75c877edaa5d13d
  Public key (hex): 0x677e73d451feffdc46384fbc0688ab08cb8c66c660b8e598aa8bbbb6c38b7695
  Account ID:       0x677e73d451feffdc46384fbc0688ab08cb8c66c660b8e598aa8bbbb6c38b7695
  SS58 Address:     5EQQMTTz6a4686C86NsC55LTFKXYUA8KAx1KythaW8q67am2
),

Account Details (
Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//3//controller` is account:
  Secret seed:      0xa0f4c3fb8efc7395eab2e0838843ce07e7c8440804792888da843ea43b3dcc56
  Public key (hex): 0x4a62568acc66d594cb24744a24968c21decb3d824ac3cde58561a30ea4e33026
  Account ID:       0x4a62568acc66d594cb24744a24968c21decb3d824ac3cde58561a30ea4e33026
  SS58 Address:     5DkEcWqE9CAj2Cusx1XSXhmLvGQQ8qChQnDJuTYMCukSjNWU

Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//3//aura` is account:
  Secret seed:      0xd771abbbab34bb3d8d93a283a64ac7ec4323c24a40a540bd8090392af5846fa5
  Public key (hex): 0x6c784086bd9a33ca691e73f14ac0fd28a71a653f7d0bf3d1c8f06efc3bc88059
  Account ID:       0x6c784086bd9a33ca691e73f14ac0fd28a71a653f7d0bf3d1c8f06efc3bc88059
  SS58 Address:     5EWvkWim11zojxkiRX9SouCAq67jKC6HiGJGEBZrPEhqMXnc

Secret Key URI `0x97dfdb567407b61f482381afe707f7c09c2a7951a2543a448217211e31a99ac4//3//grandpa` is account:
  Secret seed:      0x9a135d64f6990f77be7d1222bf5594d730f13382f6977b5e7b160453c49cf032
  Public key (hex): 0xf5ccb2eedfa6adb37bae3f28aa388c5e1e4e87b0684f77020e89879a2299fbf3
  Account ID:       0xf5ccb2eedfa6adb37bae3f28aa388c5e1e4e87b0684f77020e89879a2299fbf3
  SS58 Address:     5HczPzT6FTLYBD6rNB85TgZUpa5tupRUmkJSd15xQrtyCwba
),
```

### Chain Spec Configuration

``` bash
(
//5DWfCZZiszyGPHugjyUfyKWXbYuXFCkCUUYJJfiiL8SXUW1P
hex!["4007ab701556266c5ae6cd5306a7893146312a5ae0cfe129fdc8cbecc8763f49"].into(),
//5FgrrDx5H3jx38Xe3mD883FPFdRmDyEEoNLA3nMbPd5uzt26
hex!["a047ce4974347c3972ae4a2292ba1fad2b4ece9d6eaffd5286c185985dc92401"].unchecked_into(),
//5C9dfj4zZdFVg8ndFEXzKwiqd9FtYiracERZStwNEXefWZfv
hex!["03c22070787b00627c5817c34451e10a181f5c257dc44d49d0c6a9e46581ba27"].unchecked_into(),
),
(
//5DhYRwjpfvxYnBVD2QbPWrK5s9aJeji33nVNC99jQgwGSWpM
hex!["485494ec577ef5807683a52144343112af183ae54b95e349f0a4b122d3428014"].into(),
//5Gmq8xxxypVMA5YUjZQvsKFw71GyucC3ZnGrLTQ35C6ifcvQ
hex!["d04e815f6b73d22e5d5948e7d05bab6c8cda3cb10e37660582fa918c61f4cc7b"].unchecked_into(),
//5EQQMTTz6a4686C86NsC55LTFKXYUA8KAx1KythaW8q67am2
hex!["677e73d451feffdc46384fbc0688ab08cb8c66c660b8e598aa8bbbb6c38b7695"].unchecked_into(),
),
(
//5DkEcWqE9CAj2Cusx1XSXhmLvGQQ8qChQnDJuTYMCukSjNWU
hex!["4a62568acc66d594cb24744a24968c21decb3d824ac3cde58561a30ea4e33026"].into(),
//5EWvkWim11zojxkiRX9SouCAq67jKC6HiGJGEBZrPEhqMXnc
hex!["6c784086bd9a33ca691e73f14ac0fd28a71a653f7d0bf3d1c8f06efc3bc88059"].unchecked_into(),
//5HczPzT6FTLYBD6rNB85TgZUpa5tupRUmkJSd15xQrtyCwba
hex!["f5ccb2eedfa6adb37bae3f28aa388c5e1e4e87b0684f77020e89879a2299fbf3"].unchecked_into(),
),
```
