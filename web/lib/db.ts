import postgres from 'postgres';
let connection: ReturnType<typeof postgres> | undefined;
export function db() {
 if (!process.env.DATABASE_URL) throw new Error('Database is not configured');
 return connection ??= postgres(process.env.DATABASE_URL, {prepare:false,max:3,connect_timeout:10,idle_timeout:20});
}
