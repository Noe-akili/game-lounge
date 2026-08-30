import { neon } from '@neondatabase/serverless';
import dotenv from 'dotenv';
import bcrypt from 'bcryptjs';
dotenv.config({ path: 'server/.env' });
const sql = neon(process.env.DATABASE_URL!);
const rows = await sql`SELECT * FROM users WHERE email = 'john@gamelounge.com'`;
console.log(rows);
for (const u of rows) {
  console.log('check', bcrypt.compareSync('employe123', u.password_hash));
  console.log('hash', u.password_hash.slice(0,20));
}
