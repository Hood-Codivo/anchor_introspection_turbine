import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { DiceGame } from "../target/types/dice_game";

import {
  Transaction,
  Ed25519Program,
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { createHash, randomBytes } from "crypto";
import { BN } from "bn.js";

const BET_ROLL = 50; // we bet no 50
const BET_AMOUNT = BigInt(LAMPORTS_PER_SOL / 10); // we bet 0.1 SOL (minimum allowed)
const HOUSE_EDGE_BASIS_POINTS = BigInt(150); // 1.5%

import { assert } from "chai";

describe("dice-game", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.diceGame as Program<DiceGame>;
  const house = Keypair.generate();
  const player = Keypair.generate();
  const seed = new BN(randomBytes(16));
  const vault = PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), house.publicKey.toBuffer()],
    program.programId,
  )[0];

  const bet = PublicKey.findProgramAddressSync(
    [
      Buffer.from("bet"),
      vault.toBuffer(),
      player.publicKey.toBuffer(),
      seed.toBuffer("le", 16),
    ],
    program.programId,
  )[0];
  it("Airdrop", async () => {
    await Promise.all(
      [house, player].map(async (key) => {
        return await anchor
          .getProvider()
          .connection.requestAirdrop(
            key.publicKey,
            1000 * anchor.web3.LAMPORTS_PER_SOL,
          )
          .then(confirmTx);
      }),
    );
  });

  it("Initialize", async () => {
    await program.methods
      .initialize(new BN(100 * LAMPORTS_PER_SOL))
      .accountsStrict({
        house: house.publicKey,
        vault,
        systemProgram: SystemProgram.programId,
      })
      .signers([house])
      .rpc()
      .then(confirmTx);
  });

  it("Place a bet", async () => {
    await program.methods
      .placeBet(seed, BET_ROLL, new BN(BET_AMOUNT.toString()))
      .accountsStrict({
        player: player.publicKey,
        house: house.publicKey,
        vault,
        bet,
        systemProgram: SystemProgram.programId,
      })
      .signers([player])
      .rpc()
      .then(confirmTx);
  });

  it("Resolve a bet", async () => {
    //Pull the Bet pda
    const betpda = await anchor
      .getProvider()
      .connection.getAccountInfo(bet, "confirmed");
    if (!betpda) {
      throw new Error("Bet account not found");
    }

    const sig_ix = Ed25519Program.createInstructionWithPrivateKey({
      privateKey: house.secretKey,
      message: betpda.data.subarray(8),
    });

    //  The Ed25519 instruction data is packed like this for one signature:
    // [0..16]: header with offsets, [16..48]: public key,
    // [48..122]: signature, [112..end]: signed message.
    // The on-onchain program receives only the 64-bytes signature here, then
    // reads this full Ed25519 instruction from the instructions sysvar.
    const ed25519Signature = Buffer.from(
      sig_ix.data.subarray(16 + 32, 16 + 32 + 64),
    );

    const resolve_ix = await program.methods
      .resolveBet(ed25519Signature)
      .accountsStrict({
        player: player.publicKey,
        house: house.publicKey,
        vault,
        bet,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .signers([house])
      .instruction();

    const tx = new Transaction().add(sig_ix).add(resolve_ix);

    try {
      await sendAndConfirmTransaction(program.provider.connection, tx, [house]);
      logBetSummary("web.js", BET_ROLL, BET_AMOUNT, ed25519Signature);
    } catch (error) {
      console.error(error);
      throw error;
    }
  });
});

const confirmTx = async (signature: string): Promise<string> => {
  const latestBlockchain = await anchor
    .getProvider()
    .connection.getLatestBlockhash();
  await anchor.getProvider().connection.confirmTransaction(
    {
      signature,
      ...latestBlockchain,
    },
    "confirmed",
  );
  return signature;
};

function logBetSummary(
  label: string,
  targetRoll: number,
  amount: bigint,
  signature: Uint8Array,
) {
  const roll = resolveRoll(signature);
  const won = targetRoll > roll;
  const payout = won ? calculatePayout(amount, targetRoll) : 0n;

  console.log(
    `[${label}] Bet summary: target < ${targetRoll}, resolved roll ${roll}, ` +
      `${won ? "WON" : "LOST"}, payout ${formatSol(
        payout,
      )} SOL (${payout} lamports)`,
  );
}

function resolveRoll(signature: Uint8Array): number {
  const hash = createHash("sha256").update(signature).digest();

  const lower = bufferToBigIntLE(hash.subarray(0, 16));
  const upper = bufferToBigIntLE(hash.subarray(16, 32));

  const U128_MODULUS = 1n << 128n;
  const sum = (lower + upper) % U128_MODULUS;

  return Number(sum % 100n) + 1;
}

function bufferToBigIntLE(buf: Buffer): bigint {
  let result = 0n;
  for (let i = buf.length - 1; i >= 0; i--) {
    result = (result << 8n) + BigInt(buf[i]);
  }
  return result;
}

function calculatePayout(amount: bigint, targetRoll: number): bigint {
  const winningNumbers = BigInt(targetRoll - 1);
  return (amount * (10_000n - HOUSE_EDGE_BASIS_POINTS)) / winningNumbers / 100n;
}

function formatSol(lamports: bigint): string {
  return (Number(lamports) / LAMPORTS_PER_SOL).toFixed(4);
}
