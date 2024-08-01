# Decentralised Directories [ DeDir ] 

Design documentation of Decentralised Directories i.e DeDir on CORD Blockchain.

### Brief Intro of requirement of DeDir:

### Introduction to Digital Trust and Electronic Registries

Role of Directories in Digital Trust: In today's digital landscape, trust hinges on the use of directories, which are electronic registries containing publicly available information. These directories serve as authoritative sources of truth for public listings, such as valid banks or accredited colleges, and are essential for verifying the authenticity of credentials and claims.

Verification Against Public Information: Directories enable third parties to verify the legitimacy of entities and credentials by cross-referencing publicly available data. For instance, they help determine whether a bank issuing a deposit is licensed or if a university awarding an academic certificate is recognised within its jurisdiction.

### Benefits of Blockchain-based Registries

Enhanced Security and Integrity: Blockchain technology ensures that once information is added to the registry, it cannot be altered or deleted, providing a tamper-proof and immutable record. This significantly enhances the security and integrity of the data, making it highly trustworthy.

Increased Transparency and Accessibility: Blockchain-based registries are decentralized and transparent, allowing all participants in the network to access and verify the information independently. This openness ensures that the data is publicly available and can be easily verified by third parties, reducing the risk of fraud and enhancing overall trust in the system.

--- 

### Dispatchable Functions

#### Dispatchable Functions

 * `create_registry` - Create a registry, with states supported and entry types.
 * `create_registry_entry` - Create a registry entry for the created registry. 
 * `registry_entry_status_change` - Change the status of the registry entry.
 