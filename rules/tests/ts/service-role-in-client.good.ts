"use client";
import { createClient } from "@supabase/supabase-js";
export const anon = createClient("https://x.supabase.co", process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY as string);
