import "@stdlib/deploy";

// --- TEP-74 Jetton Standards ---

message(0x17354605) TokenTransferInternal {
    query_id: Int as uint64;
    amount: Int as coins;
    from: Address;
    response_destination: Address;
    forward_ton_amount: Int as coins;
    forward_payload: Slice as remaining;
}

message(0x7bdd97de) TokenBurnNotification {
    query_id: Int as uint64;
    amount: Int as coins;
    sender: Address;
    response_destination: Address;
}

// --- Bridge Specific Messages ---

message Mint {
    amount: Int as coins;
    receiver: Address;
    burn_id: Int as uint256; // The unique ID or Hash from the SEER internal chain
}

contract SeerJettonMaster with Deployable {
    total_supply: Int as coins = 0;
    admin: Address;
    content: Cell;
    processed_burns: map<Int as uint256, Bool>;

    init(admin: Address, content: Cell) {
        self.admin = admin;
        self.content = content;
    }

    // --- Minting Logic ---

    receive(msg: Mint) {
        let ctx: Context = context();
        require(ctx.sender == self.admin, "Only admin (Relayer) can mint");
        require(self.processed_burns.get(msg.burn_id) == null, "Burn ID already processed");

        // Mark as processed
        self.processed_burns.set(msg.burn_id, true);

        // Update supply
        self.total_supply = self.total_supply + msg.amount;

        // Deploy/Mint to user's wallet
        let init: StateInit = self.getJettonWalletInit(msg.receiver);
        send(SendParameters{
            to: contractAddress(init),
            value: 0,
            bounce: true,
            mode: SendRemainingValue,
            body: TokenTransferInternal{
                query_id: 0,
                amount: msg.amount,
                from: self.address,
                response_destination: self.admin,
                forward_ton_amount: 0,
                forward_payload: emptySlice()
            }.toCell(),
            code: init.code,
            data: init.data
        });
    }

    // --- Getters ---

    get fun get_jetton_data(): JettonData {
        return JettonData{
            total_supply: self.total_supply,
            mintable: true,
            admin_address: self.admin,
            content: self.content,
            wallet_code: self.getJettonWalletInit(myAddress()).code
        };
    }

    get fun get_wallet_address(owner_address: Address): Address {
        let init: StateInit = self.getJettonWalletInit(owner_address);
        return contractAddress(init);
    }

    // --- Internal Helpers ---

    fun getJettonWalletInit(address: Address): StateInit {
        return initOf SeerJettonWallet(myAddress(), address);
    }
}

struct JettonData {
    total_supply: Int;
    mintable: Bool;
    admin_address: Address;
    content: Cell;
    wallet_code: Cell;
}

// --- Jetton Wallet Contract ---

contract SeerJettonWallet {
    balance: Int as coins = 0;
    owner: Address;
    master: Address;

    init(master: Address, owner: Address) {
        self.master = master;
        self.owner = owner;
    }

    receive(msg: TokenTransferInternal) {
        let ctx: Context = context();
        if (ctx.sender != self.master) {
            let init: StateInit = initOf SeerJettonWallet(self.master, msg.from);
            require(ctx.sender == contractAddress(init), "Invalid sender");
        }

        self.balance = self.balance + msg.amount;
    }

    receive("burn") {
        // Simple internal burn (can be expanded for TEP-74)
        require(context().sender == self.owner, "Invalid owner");
        self.balance = 0;
    }

    get fun get_wallet_data(): WalletData {
        return WalletData{
            balance: self.balance,
            owner: self.owner,
            master: self.master,
            wallet_code: (initOf SeerJettonWallet(self.master, self.owner)).code
        };
    }
}

struct WalletData {
    balance: Int;
    owner: Address;
    master: Address;
    wallet_code: Cell;
}
