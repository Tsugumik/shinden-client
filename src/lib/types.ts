export type Anime = {
    name: string,
    url: string,
    image_url: string,
    anime_type: string,
    rating: string,
    episodes: string,
    description: string,
}

export type User = {
    name: string | null,
    image_url: string | null,
}

export type Episode = {
    title: string,
    link: string,
}

export type Player = {
    player: string,
    max_res: string,
    lang_audio: string,
    lang_subs: string,
    online_id: string,
}