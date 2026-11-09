"use client";
import { createClient } from "@supabase/supabase-js";
export const admin = createClient("https://x.supabase.co", process.env.SUPABASE_SERVICE_ROLE_KEY as string);
