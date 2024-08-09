import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Tokensale } from "../target/types/tokensale";
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import { AccountLayout, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, createMint, getAccount, getMint, getOrCreateAssociatedTokenAccount, mintTo, getAssociatedTokenAddress } from '@solana/spl-token';
import BN from 'bn.js'
import { assert } from "chai";


describe("tokensale", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Tokensale as Program<Tokensale>;
  const connection = new Connection("http://127.0.0.1:8899", "confirmed");

  const adminKeypair = provider.wallet as anchor.Wallet;
  const rdmUser = Keypair.generate();

  let token_price = 0.05;
  let purchase_limit = 400;
  let decimals = 9;

  let mint_pubkey: PublicKey;

  it("Create token mint and mint some", async () => {
    await connection.requestAirdrop(rdmUser.publicKey, 1e9);

    mint_pubkey = await createMint(
      connection,
      adminKeypair.payer,
      adminKeypair.publicKey,
      adminKeypair.publicKey,
      decimals // We are using 9 to match the CLI decimal default exactly
    );

    const tokenAccount = await getOrCreateAssociatedTokenAccount(
      connection,
      adminKeypair.payer,
      mint_pubkey,
      adminKeypair.publicKey
    );

    await mintTo(
      connection,
      adminKeypair.payer,
      mint_pubkey,
      tokenAccount.address,
      adminKeypair.payer,
      1000 * Math.pow(10, decimals)
    );

    const tokenAccountInfo = await getAccount(
      connection,
      tokenAccount.address
    );

    assert(Number(tokenAccountInfo.amount) == (1000 * Math.pow(10, decimals)), "Admin ATA should have 250 tokens, that we just minted to him");



  });

  let [configPublicKey] = PublicKey.findProgramAddressSync([Buffer.from("CONFIG_ACCOUNT")], program.programId);
  let [tokenAccountOwnerPda] = PublicKey.findProgramAddressSync([Buffer.from("token_account_owner_pda")], program.programId);
  let [programTokenAccount] = PublicKey.findProgramAddressSync([Buffer.from("PROGRAM_TOKEN_ACCOUNT")], program.programId);

  it("Initialisation", async () => {
    let tx = await program.methods
      .initialize(new BN(token_price * Math.pow(10, decimals)), new BN(purchase_limit * Math.pow(10, decimals)), adminKeypair.publicKey)
      .accounts({
        configAccount: configPublicKey,
        signer: adminKeypair.publicKey,
        systemProgram: SystemProgram.programId,
        tokenMint: mint_pubkey,
        tokenAccountOwnerPda: tokenAccountOwnerPda,
        programTokenAccount: programTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .rpc();
    await connection.confirmTransaction(tx);
    let configData = await program.account.configurationAccount.fetch(configPublicKey);

    assert(configData.adminPubkey.toString() == adminKeypair.publicKey.toString(), "Admin pubkey on the PDA does not match expected value");
    assert(configData.tokenPrice.toNumber() / Math.pow(10, decimals) == token_price, "Token price on the PDA does not match expected value");
    assert(configData.purchaseLimit.toNumber() / Math.pow(10, decimals) == purchase_limit, "Purchase limit on the PDA does not match expected value");


  });

  it("Deposit", async () => {
    let admin_ata = await getAssociatedTokenAddress(mint_pubkey, adminKeypair.publicKey);


    let tx_id = await program.methods
      .deposit(new BN(1000 * Math.pow(10, 9)))
      .accounts({
        signer: adminKeypair.publicKey,
        signerAta: admin_ata,
        tokenMint: mint_pubkey,
        tokenAccountOwnerPda: tokenAccountOwnerPda,
        programTokenAccount: programTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,

      })
      .rpc();
    await connection.confirmTransaction(tx_id);

    const programTokenAccountInfo = await getAccount(
      connection,
      programTokenAccount
    );

    const adminTokenAccountInfo = await getAccount(
      connection,
      admin_ata
    );
    assert(Number(adminTokenAccountInfo.amount) == (0), "Admin ata should have 0 tokens left");
    assert(Number(programTokenAccountInfo.amount) == (1000 * Math.pow(10, decimals)), "Program token account should now have 1000 tokens");


  });




  it("Buy without being whitelisted", async () => {
    let [userAccount] = PublicKey.findProgramAddressSync([Buffer.from("USER_ACCOUNT"), rdmUser.publicKey.toBuffer()], program.programId);
    let user_ata = await getAssociatedTokenAddress(mint_pubkey, rdmUser.publicKey);
    try {
      let tx_id = await program.methods
        .buyToken(new BN(3 * Math.pow(10, 9)))
        .accounts({
          configAccount: configPublicKey,
          signer: rdmUser.publicKey,
          userAccount: userAccount,
          userAta: user_ata,
          systemProgram: SystemProgram.programId,
          tokenMint: mint_pubkey,
          tokenAccountOwnerPda: tokenAccountOwnerPda,
          programTokenAccount: programTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        })
        .signers([rdmUser])
        .rpc()

      // We should be in the catch statement since we're not whitelisted
      assert(false, "The transaction should fail since the user is not whitelisted");
    }
    catch (e) {
      assert(e.error.errorCode.code == "AccountNotInitialized", "Buying without being whitelisted should raise an error")
    }

  });

  it("Whitelist", async () => {
    let [userAccount] = PublicKey.findProgramAddressSync([Buffer.from("USER_ACCOUNT"), rdmUser.publicKey.toBuffer()], program.programId);

    let tx_id = await program.methods
      .addToWhitelist(rdmUser.publicKey)
      .accounts({
        configAccount: configPublicKey,
        signer: adminKeypair.publicKey,
        userAccount: userAccount,
        systemProgram: SystemProgram.programId,
      })
      .rpc()




  });

  it("Buy being whitelisted", async () => {
    let tx = await connection.requestAirdrop(rdmUser.publicKey, 1e9);
    await connection.confirmTransaction(tx);

    let [userAccount] = PublicKey.findProgramAddressSync([Buffer.from("USER_ACCOUNT"), rdmUser.publicKey.toBuffer()], program.programId);
    let user_ata = await getAssociatedTokenAddress(mint_pubkey, rdmUser.publicKey);

    let tx_id = await program.methods
      .buyToken(new BN(3 * Math.pow(10, 9)))
      .accounts({
        configAccount: configPublicKey,
        signer: rdmUser.publicKey,
        userAccount: userAccount,
        userAta: user_ata,
        systemProgram: SystemProgram.programId,
        tokenMint: mint_pubkey,
        tokenAccountOwnerPda: tokenAccountOwnerPda,
        programTokenAccount: programTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .signers([rdmUser])
      .rpc();
    await connection.confirmTransaction(tx_id);


    const userAtaInfo = await getAccount(
      connection,
      user_ata
    );
    assert(Number(userAtaInfo.amount) == (3 * Math.pow(10, 9)));

  });

  it("Admin withdraw SOL", async () => {
    let balance_on_program = await connection.getBalance(tokenAccountOwnerPda);
    let amount_to_withdraw = Math.floor((parseFloat((balance_on_program / LAMPORTS_PER_SOL).toFixed(4)) * LAMPORTS_PER_SOL) / 2);

    let tx_id = await program.methods
      .withdraw(new BN(amount_to_withdraw))
      .accounts({
        signer: adminKeypair.publicKey,
        configAccount: configPublicKey,
        tokenAccountOwnerPda: tokenAccountOwnerPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    await connection.confirmTransaction(tx_id);


  });
});




